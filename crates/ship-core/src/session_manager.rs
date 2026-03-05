use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use ship_types::{
    AgentSnapshot, AgentState, AutonomyMode, BlockId, BlockPatch, CloseSessionResponse,
    ContentBlock, CreateSessionRequest, CurrentTask, PermissionRequest, PermissionResolution,
    PersistedSession, Role, SessionConfig, SessionEvent, SessionEventEnvelope, SessionId,
    SessionSummary, TaskContentRecord, TaskId, TaskRecord, TaskStatus,
};
use tokio::sync::broadcast;

use crate::{AgentDriver, SessionStore, StopReason, WorktreeOps};

#[derive(Debug)]
pub enum SessionManagerError {
    SessionNotFound(SessionId),
    NoActiveTask,
    ActiveTaskAlreadyExists,
    InvalidTaskTransition { from: TaskStatus, to: TaskStatus },
    Agent(String),
    Worktree(String),
    Store(String),
    PermissionNotFound(String),
}

impl fmt::Display for SessionManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionNotFound(id) => write!(f, "session not found: {}", id.0),
            Self::NoActiveTask => f.write_str("session has no active task"),
            Self::ActiveTaskAlreadyExists => {
                f.write_str("session already has an active non-terminal task")
            }
            Self::InvalidTaskTransition { from, to } => {
                write!(f, "invalid task transition from {from:?} to {to:?}")
            }
            Self::Agent(message) => write!(f, "agent error: {message}"),
            Self::Worktree(message) => write!(f, "worktree error: {message}"),
            Self::Store(message) => write!(f, "store error: {message}"),
            Self::PermissionNotFound(permission_id) => {
                write!(f, "permission not found: {permission_id}")
            }
        }
    }
}

impl std::error::Error for SessionManagerError {}

#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub block_id: BlockId,
    pub role: Role,
    pub tool_name: String,
    pub arguments: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub id: SessionId,
    pub config: SessionConfig,
    pub worktree_path: PathBuf,
    pub captain_handle: crate::AgentHandle,
    pub mate_handle: crate::AgentHandle,
    pub captain: AgentSnapshot,
    pub mate: AgentSnapshot,
    pub current_task: Option<CurrentTask>,
    pub task_history: Vec<TaskRecord>,
    pub captain_block_count: usize,
    pub mate_block_count: usize,
    pub pending_permissions: HashMap<String, PendingPermission>,
    pub pending_steer: Option<String>,
    pub events_tx: broadcast::Sender<SessionEventEnvelope>,
    pub next_event_seq: u64,
}

#[derive(Debug, Clone)]
pub struct SessionStateView {
    pub id: SessionId,
    pub config: SessionConfig,
    pub captain: AgentSnapshot,
    pub mate: AgentSnapshot,
    pub current_task: Option<CurrentTask>,
    pub task_history: Vec<TaskRecord>,
    pub autonomy_mode: AutonomyMode,
    pub pending_permissions: Vec<PendingPermission>,
}

pub struct SessionManager<A: AgentDriver, W: WorktreeOps, S: SessionStore> {
    agent_driver: A,
    worktree_ops: W,
    store: S,
    sessions: HashMap<SessionId, ActiveSession>,
}

impl<A: AgentDriver, W: WorktreeOps, S: SessionStore> SessionManager<A, W, S> {
    pub fn new(agent_driver: A, worktree_ops: W, store: S) -> Self {
        Self {
            agent_driver,
            worktree_ops,
            store,
            sessions: HashMap::new(),
        }
    }

    // r[proto.create-session]
    pub async fn create_session(
        &mut self,
        req: CreateSessionRequest,
        repo_root: &Path,
    ) -> Result<(SessionId, TaskId), SessionManagerError> {
        let session_id = SessionId::new();
        let slug = slugify(&req.task_description);
        let worktree_path = self
            .worktree_ops
            .create_worktree(&session_id, &req.base_branch, &slug, repo_root)
            .await
            .map_err(|error| SessionManagerError::Worktree(error.message))?;

        let branch_name = format!("ship/{}/{slug}", short_id(&session_id));

        let captain_handle = self
            .agent_driver
            .spawn(req.captain_kind, Role::Captain, &worktree_path)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;

        let mate_handle = self
            .agent_driver
            .spawn(req.mate_kind, Role::Mate, &worktree_path)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;

        let (events_tx, _) = broadcast::channel(256);

        let session = ActiveSession {
            id: session_id.clone(),
            config: SessionConfig {
                project: req.project,
                base_branch: req.base_branch,
                branch_name,
                captain_kind: req.captain_kind,
                mate_kind: req.mate_kind,
                autonomy_mode: AutonomyMode::HumanInTheLoop,
            },
            worktree_path,
            captain_handle,
            mate_handle,
            captain: AgentSnapshot {
                role: Role::Captain,
                kind: req.captain_kind,
                state: AgentState::Idle,
                context_remaining_percent: None,
            },
            mate: AgentSnapshot {
                role: Role::Mate,
                kind: req.mate_kind,
                state: AgentState::Idle,
                context_remaining_percent: None,
            },
            current_task: None,
            task_history: Vec::new(),
            captain_block_count: 0,
            mate_block_count: 0,
            pending_permissions: HashMap::new(),
            pending_steer: None,
            events_tx,
            next_event_seq: 0,
        };

        self.sessions.insert(session_id.clone(), session);

        let task_id = self.assign(&session_id, req.task_description).await?;
        Ok((session_id, task_id))
    }

    // r[proto.list-sessions]
    pub fn list_sessions(&self) -> Vec<SessionSummary> {
        self.sessions
            .values()
            .map(|session| SessionSummary {
                id: session.id.clone(),
                project: session.config.project.clone(),
                branch_name: session.config.branch_name.clone(),
                captain: session.captain.clone(),
                mate: session.mate.clone(),
                current_task_description: session
                    .current_task
                    .as_ref()
                    .map(|task| task.record.description.clone()),
                task_status: session.current_task.as_ref().map(|task| task.record.status),
                autonomy_mode: session.config.autonomy_mode,
            })
            .collect()
    }

    // r[proto.get-session]
    pub fn get_session(
        &self,
        session_id: &SessionId,
    ) -> Result<SessionStateView, SessionManagerError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

        Ok(SessionStateView {
            id: session.id.clone(),
            config: session.config.clone(),
            captain: session.captain.clone(),
            mate: session.mate.clone(),
            current_task: session.current_task.clone(),
            task_history: session.task_history.clone(),
            autonomy_mode: session.config.autonomy_mode,
            pending_permissions: session.pending_permissions.values().cloned().collect(),
        })
    }

    // r[event.subscribe.roam-channel]
    // r[event.subscribe.replay]
    pub fn subscribe(
        &self,
        session_id: &SessionId,
    ) -> Result<broadcast::Receiver<SessionEventEnvelope>, SessionManagerError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

        let rx = session.events_tx.subscribe();

        if let Some(task) = session.current_task.as_ref() {
            for envelope in &task.event_log {
                let _ = session.events_tx.send(envelope.clone());
            }
        }

        Ok(rx)
    }

    // r[autonomy.toggle]
    pub fn set_autonomy_mode(
        &mut self,
        session_id: &SessionId,
        mode: AutonomyMode,
    ) -> Result<(), SessionManagerError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

        session.config.autonomy_mode = mode;
        Ok(())
    }

    // r[proto.assign]
    pub async fn assign(
        &mut self,
        session_id: &SessionId,
        description: String,
    ) -> Result<TaskId, SessionManagerError> {
        {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            if session.current_task.is_some() {
                return Err(SessionManagerError::ActiveTaskAlreadyExists);
            }
        }

        let task_id = TaskId::new();
        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

            session.current_task = Some(CurrentTask {
                record: TaskRecord {
                    id: task_id.clone(),
                    description: description.clone(),
                    status: TaskStatus::Assigned,
                },
                content_history: Vec::new(),
                event_log: Vec::new(),
            });

            apply_event(
                session,
                SessionEvent::TaskStarted {
                    task_id: task_id.clone(),
                    description: description.clone(),
                },
            );
            apply_event(
                session,
                SessionEvent::TaskStatusChanged {
                    task_id: task_id.clone(),
                    status: TaskStatus::Assigned,
                },
            );
        }

        self.persist_session(session_id).await?;

        self.prompt_captain_initial(session_id, &description)
            .await?;

        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            transition_task(session, TaskStatus::Working)?;
        }

        self.persist_session(session_id).await?;

        self.prompt_mate(
            session_id,
            format!(
                "Task:\n{description}\n\nCaptain direction:\nProvide an implementation plan and execute."
            ),
        )
        .await?;

        Ok(task_id)
    }

    // r[proto.steer]
    pub async fn steer(
        &mut self,
        session_id: &SessionId,
        message: String,
    ) -> Result<(), SessionManagerError> {
        let mode = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

            let status = current_task_status(session)?;
            if status != TaskStatus::ReviewPending && status != TaskStatus::SteerPending {
                return Err(SessionManagerError::InvalidTaskTransition {
                    from: status,
                    to: TaskStatus::SteerPending,
                });
            }

            apply_event(
                session,
                SessionEvent::BlockAppend {
                    block_id: BlockId::new(),
                    role: Role::Captain,
                    block: ContentBlock::Text {
                        text: message.clone(),
                    },
                },
            );

            session.config.autonomy_mode
        };

        if mode == AutonomyMode::Autonomous {
            {
                let session = self
                    .sessions
                    .get_mut(session_id)
                    .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
                transition_task(session, TaskStatus::Working)?;
            }

            self.prompt_mate(session_id, format!("Captain steer:\n{message}"))
                .await?;
        } else {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            transition_task(session, TaskStatus::SteerPending)?;
            session.pending_steer = Some(message);
        }

        self.persist_session(session_id).await?;
        Ok(())
    }

    // r[proto.accept]
    pub async fn accept(&mut self, session_id: &SessionId) -> Result<(), SessionManagerError> {
        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            let status = current_task_status(session)?;
            if status != TaskStatus::ReviewPending {
                return Err(SessionManagerError::InvalidTaskTransition {
                    from: status,
                    to: TaskStatus::Accepted,
                });
            }

            transition_task(session, TaskStatus::Accepted)?;
            archive_terminal_task(session);
        }

        self.persist_session(session_id).await?;
        Ok(())
    }

    // r[proto.cancel]
    pub async fn cancel(&mut self, session_id: &SessionId) -> Result<(), SessionManagerError> {
        let handle = {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            if session
                .current_task
                .as_ref()
                .map(|task| task.record.status.is_terminal())
                .unwrap_or(true)
            {
                return Err(SessionManagerError::NoActiveTask);
            }
            session.mate_handle.clone()
        };

        self.agent_driver
            .cancel(&handle)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;

        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            transition_task(session, TaskStatus::Cancelled)?;
            archive_terminal_task(session);
        }

        self.persist_session(session_id).await?;
        Ok(())
    }

    // r[proto.resolve-permission]
    pub async fn resolve_permission(
        &mut self,
        session_id: &SessionId,
        permission_id: &str,
        approved: bool,
    ) -> Result<(), SessionManagerError> {
        let (pending, handle) = {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

            let pending = session
                .pending_permissions
                .get(permission_id)
                .cloned()
                .ok_or_else(|| SessionManagerError::PermissionNotFound(permission_id.to_owned()))?;

            let handle = match pending.role {
                Role::Captain => session.captain_handle.clone(),
                Role::Mate => session.mate_handle.clone(),
            };

            (pending, handle)
        };

        self.agent_driver
            .resolve_permission(&handle, permission_id, approved)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

        set_agent_state(
            session,
            pending.role,
            AgentState::Working {
                plan: None,
                activity: Some("Permission resolved".to_owned()),
            },
        );
        apply_event(
            session,
            SessionEvent::BlockPatch {
                block_id: pending.block_id,
                role: pending.role,
                patch: BlockPatch::PermissionResolve {
                    resolution: if approved {
                        PermissionResolution::Approved
                    } else {
                        PermissionResolution::Denied
                    },
                },
            },
        );

        Ok(())
    }

    fn repo_root_for_worktree(worktree_path: &Path) -> Result<&Path, SessionManagerError> {
        worktree_path
            .parent()
            .and_then(|parent| parent.parent())
            .ok_or_else(|| {
                SessionManagerError::Worktree(format!(
                    "invalid worktree path: {}",
                    worktree_path.display()
                ))
            })
    }

    // r[proto.close-session]
    // r[backend.worktree-management]
    // r[worktree.cleanup]
    // r[worktree.cleanup-uncommitted]
    // r[worktree.cleanup-git]
    pub async fn close_session(
        &mut self,
        session_id: &SessionId,
        force: bool,
    ) -> Result<CloseSessionResponse, SessionManagerError> {
        let worktree_path = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?
            .worktree_path
            .clone();

        if self
            .worktree_ops
            .has_uncommitted_changes(&worktree_path)
            .await
            .map_err(|error| SessionManagerError::Worktree(error.message))?
            && !force
        {
            return Ok(CloseSessionResponse::RequiresConfirmation);
        }

        let session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
        let repo_root = Self::repo_root_for_worktree(&session.worktree_path)?;

        self.agent_driver
            .kill(&session.captain_handle)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message.clone()))?;
        self.agent_driver
            .kill(&session.mate_handle)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message.clone()))?;
        self.worktree_ops
            .remove_worktree(&session.worktree_path, force)
            .await
            .map_err(|error| SessionManagerError::Worktree(error.message))?;
        self.worktree_ops
            .delete_branch(&session.config.branch_name, force, repo_root)
            .await
            .map_err(|error| SessionManagerError::Worktree(error.message))?;

        Ok(CloseSessionResponse::Closed)
    }

    pub async fn drain_notifications(
        &mut self,
        session_id: &SessionId,
        role: Role,
    ) -> Result<(), SessionManagerError> {
        let handle = {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            match role {
                Role::Captain => session.captain_handle.clone(),
                Role::Mate => session.mate_handle.clone(),
            }
        };

        let mut stream = self.agent_driver.notifications(&handle);
        while let Some(event) = stream.next().await {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            apply_event(session, event);
        }
        drop(stream);

        self.persist_session(session_id).await?;
        Ok(())
    }

    // r[captain.initial-prompt]
    async fn prompt_captain_initial(
        &mut self,
        session_id: &SessionId,
        task_description: &str,
    ) -> Result<(), SessionManagerError> {
        self.prompt_agent(
            session_id,
            Role::Captain,
            format!("Provide implementation direction for this task:\n{task_description}"),
        )
        .await
        .map(|_| ())
    }

    // r[captain.review-trigger]
    async fn prompt_captain_review(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), SessionManagerError> {
        let prompt = {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            let task = session
                .current_task
                .as_ref()
                .ok_or(SessionManagerError::NoActiveTask)?;
            format!(
                "Review mate output for task:\n{}\n\nProvide steer or acceptance.",
                task.record.description
            )
        };

        self.prompt_agent(session_id, Role::Captain, prompt)
            .await
            .map(|_| ())
    }

    async fn prompt_mate(
        &mut self,
        session_id: &SessionId,
        prompt: String,
    ) -> Result<(), SessionManagerError> {
        let stop_reason = self.prompt_agent(session_id, Role::Mate, prompt).await?;

        match stop_reason {
            StopReason::EndTurn => {
                {
                    let session = self
                        .sessions
                        .get_mut(session_id)
                        .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
                    transition_task(session, TaskStatus::ReviewPending)?;
                }
                self.prompt_captain_review(session_id).await?;
            }
            StopReason::Cancelled => {
                let session = self
                    .sessions
                    .get_mut(session_id)
                    .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
                transition_task(session, TaskStatus::Cancelled)?;
                archive_terminal_task(session);
            }
            StopReason::ContextExhausted => {
                let session = self
                    .sessions
                    .get_mut(session_id)
                    .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
                set_agent_state(session, Role::Mate, AgentState::ContextExhausted);
            }
        }

        Ok(())
    }

    async fn prompt_agent(
        &mut self,
        session_id: &SessionId,
        role: Role,
        prompt: String,
    ) -> Result<StopReason, SessionManagerError> {
        let handle = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            set_agent_state(
                session,
                role,
                AgentState::Working {
                    plan: None,
                    activity: Some("Prompt in progress".to_owned()),
                },
            );
            match role {
                Role::Captain => session.captain_handle.clone(),
                Role::Mate => session.mate_handle.clone(),
            }
        };

        let response = self
            .agent_driver
            .prompt(&handle, &prompt)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;

        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            match response.stop_reason {
                StopReason::ContextExhausted => {
                    set_agent_state(session, role, AgentState::ContextExhausted)
                }
                _ => set_agent_state(session, role, AgentState::Idle),
            }
        }

        self.drain_notifications(session_id, role).await?;
        self.persist_session(session_id).await?;

        Ok(response.stop_reason)
    }

    async fn persist_session(&mut self, session_id: &SessionId) -> Result<(), SessionManagerError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

        rebuild_materialized_from_event_log(session);

        // r[backend.persistence-contents]
        let persisted = PersistedSession {
            id: session.id.clone(),
            config: session.config.clone(),
            captain: session.captain.clone(),
            mate: session.mate.clone(),
            current_task: session.current_task.clone(),
            task_history: session.task_history.clone(),
        };

        self.store
            .save_session(&persisted)
            .await
            .map_err(|error| SessionManagerError::Store(error.message))?;

        Ok(())
    }
}

// r[backend.event-pipeline]
// r[backend.event-log]
pub fn apply_event(session: &mut ActiveSession, event: SessionEvent) {
    let envelope = SessionEventEnvelope {
        seq: session.next_event_seq,
        event,
    };
    session.next_event_seq = session.next_event_seq.saturating_add(1);

    if let Some(task) = session.current_task.as_mut() {
        task.event_log.push(envelope.clone());
    }

    apply_event_to_materialized_state(session, &envelope.event);
    let _ = session.events_tx.send(envelope);
}

// r[backend.materialized-state]
pub fn apply_event_to_materialized_state(session: &mut ActiveSession, event: &SessionEvent) {
    match &event {
        SessionEvent::BlockAppend {
            block_id,
            role,
            block,
        } => {
            if let Some(task) = session.current_task.as_mut() {
                task.content_history.push(TaskContentRecord {
                    block_id: block_id.clone(),
                    role: *role,
                    block: block.clone(),
                });
            }
            match role {
                Role::Captain => {
                    session.captain_block_count = session.captain_block_count.saturating_add(1)
                }
                Role::Mate => session.mate_block_count = session.mate_block_count.saturating_add(1),
            }

            if let ContentBlock::Permission {
                tool_name,
                arguments,
                description,
                ..
            } = block
            {
                let permission_id = block_id.0.to_string();
                session.pending_permissions.insert(
                    permission_id.clone(),
                    PendingPermission {
                        block_id: block_id.clone(),
                        role: *role,
                        tool_name: tool_name.clone(),
                        arguments: arguments.clone(),
                        description: description.clone(),
                    },
                );

                let state = AgentState::AwaitingPermission {
                    request: PermissionRequest {
                        permission_id,
                        tool_name: tool_name.clone(),
                        arguments: arguments.clone(),
                        description: description.clone(),
                    },
                };
                match role {
                    Role::Captain => session.captain.state = state,
                    Role::Mate => session.mate.state = state,
                }
            }
        }
        SessionEvent::BlockPatch {
            block_id,
            role,
            patch,
        } => {
            if let Some(task) = session.current_task.as_mut()
                && let Some(record) = task
                    .content_history
                    .iter_mut()
                    .find(|record| record.block_id == *block_id)
            {
                apply_block_patch(&mut record.block, patch);
            }
            if matches!(patch, BlockPatch::PermissionResolve { .. }) {
                session
                    .pending_permissions
                    .retain(|_, pending| pending.block_id != *block_id || pending.role != *role);
            }
        }
        SessionEvent::AgentStateChanged { role, state } => match role {
            Role::Captain => session.captain.state = state.clone(),
            Role::Mate => session.mate.state = state.clone(),
        },
        SessionEvent::TaskStatusChanged { task_id, status } => {
            if let Some(task) = session.current_task.as_mut()
                && task.record.id == *task_id
            {
                task.record.status = *status;
            }
        }
        SessionEvent::TaskStarted {
            task_id,
            description,
        } => {
            if let Some(task) = session.current_task.as_mut() {
                task.record.id = task_id.clone();
                task.record.description = description.clone();
            }
        }
        SessionEvent::ContextUpdated {
            role,
            remaining_percent,
        } => match role {
            Role::Captain => session.captain.context_remaining_percent = Some(*remaining_percent),
            Role::Mate => session.mate.context_remaining_percent = Some(*remaining_percent),
        },
    }
}

pub fn apply_block_patch(block: &mut ContentBlock, patch: &BlockPatch) {
    match patch {
        BlockPatch::TextAppend { text } => {
            if let ContentBlock::Text { text: existing } = block {
                existing.push_str(text);
            }
        }
        BlockPatch::ToolCallUpdate { status, result } => {
            if let ContentBlock::ToolCall {
                status: existing_status,
                result: existing_result,
                ..
            } = block
            {
                *existing_status = *status;
                *existing_result = result.clone();
            }
        }
        BlockPatch::PlanReplace { steps } => {
            if let ContentBlock::PlanUpdate { steps: existing } = block {
                *existing = steps.clone();
            }
        }
        BlockPatch::PermissionResolve { resolution } => {
            if let ContentBlock::Permission {
                resolution: existing_resolution,
                ..
            } = block
            {
                *existing_resolution = Some(*resolution);
            }
        }
    }
}

// r[backend.persistence-contents]
pub fn rebuild_materialized_from_event_log(session: &mut ActiveSession) {
    let Some(current_task) = session.current_task.as_mut() else {
        session.pending_permissions.clear();
        session.captain_block_count = 0;
        session.mate_block_count = 0;
        return;
    };

    current_task.content_history.clear();
    session.pending_permissions.clear();
    session.captain_block_count = 0;
    session.mate_block_count = 0;
    session.captain.state = AgentState::Idle;
    session.mate.state = AgentState::Idle;
    session.captain.context_remaining_percent = None;
    session.mate.context_remaining_percent = None;

    let replay = current_task.event_log.clone();
    for envelope in replay {
        apply_event_to_materialized_state(session, &envelope.event);
    }
}

// r[task.progress]
pub fn transition_task(
    session: &mut ActiveSession,
    next: TaskStatus,
) -> Result<(), SessionManagerError> {
    let task = session
        .current_task
        .as_mut()
        .ok_or(SessionManagerError::NoActiveTask)?;

    let from = task.record.status;
    if !is_valid_transition(from, next) {
        return Err(SessionManagerError::InvalidTaskTransition { from, to: next });
    }

    let task_id = task.record.id.clone();
    apply_event(
        session,
        SessionEvent::TaskStatusChanged {
            task_id,
            status: next,
        },
    );

    Ok(())
}

// r[task.completion]
pub fn is_valid_transition(from: TaskStatus, to: TaskStatus) -> bool {
    if from.is_terminal() {
        return false;
    }

    if to == TaskStatus::Cancelled {
        return true;
    }

    matches!(
        (from, to),
        (TaskStatus::Assigned, TaskStatus::Working)
            | (TaskStatus::Working, TaskStatus::ReviewPending)
            | (TaskStatus::ReviewPending, TaskStatus::SteerPending)
            | (TaskStatus::ReviewPending, TaskStatus::Working)
            | (TaskStatus::ReviewPending, TaskStatus::Accepted)
            | (TaskStatus::SteerPending, TaskStatus::Working)
    )
}

pub fn archive_terminal_task(session: &mut ActiveSession) {
    let should_archive = session
        .current_task
        .as_ref()
        .map(|task| task.record.status.is_terminal())
        .unwrap_or(false);

    if !should_archive {
        return;
    }

    if let Some(task) = session.current_task.take() {
        session.task_history.push(task.record);
    }

    session.pending_permissions.clear();
    session.pending_steer = None;
}

pub fn current_task_status(session: &ActiveSession) -> Result<TaskStatus, SessionManagerError> {
    session
        .current_task
        .as_ref()
        .map(|task| task.record.status)
        .ok_or(SessionManagerError::NoActiveTask)
}

pub fn set_agent_state(session: &mut ActiveSession, role: Role, state: AgentState) {
    apply_event(session, SessionEvent::AgentStateChanged { role, state });
}

fn short_id(id: &SessionId) -> String {
    id.0.to_string().chars().take(8).collect()
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "task".to_owned()
    } else {
        slug
    }
}
