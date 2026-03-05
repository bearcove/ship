use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use ship_types::{
    AgentSnapshot, AgentState, AutonomyMode, ContentBlock, CreateSessionRequest, CurrentTask,
    PermissionRequest, PersistedSession, Role, SessionConfig, SessionEvent, SessionId,
    SessionSummary, TaskContentRecord, TaskId, TaskRecord, TaskStatus,
};
use tokio::sync::broadcast;
use ulid::Ulid;

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
    pub role: Role,
    pub request: PermissionRequest,
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
    pub pending_permissions: HashMap<String, PendingPermission>,
    pub pending_steer: Option<String>,
    pub events_tx: broadcast::Sender<SessionEvent>,
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

    pub async fn create_session(
        &mut self,
        req: CreateSessionRequest,
        repo_root: &Path,
    ) -> Result<(SessionId, TaskId), SessionManagerError> {
        let session_id = SessionId(Ulid::new());
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
            pending_permissions: HashMap::new(),
            pending_steer: None,
            events_tx,
        };

        self.sessions.insert(session_id.clone(), session);

        let task_id = self.assign(&session_id, req.task_description).await?;
        Ok((session_id, task_id))
    }

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

    pub fn subscribe(
        &self,
        session_id: &SessionId,
    ) -> Result<broadcast::Receiver<SessionEvent>, SessionManagerError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

        Ok(session.events_tx.subscribe())
    }

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

        let task_id = TaskId(Ulid::new());
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
            });

            emit_event(
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

            emit_event(
                session,
                SessionEvent::Content {
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

    pub async fn resolve_permission(
        &mut self,
        session_id: &SessionId,
        permission_id: &str,
        _approved: bool,
    ) -> Result<(), SessionManagerError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

        let pending = session
            .pending_permissions
            .remove(permission_id)
            .ok_or_else(|| SessionManagerError::PermissionNotFound(permission_id.to_owned()))?;

        set_agent_state(
            session,
            pending.role,
            AgentState::Working {
                plan: None,
                activity: Some("Permission resolved".to_owned()),
            },
        );

        Ok(())
    }

    pub async fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), SessionManagerError> {
        let session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

        self.agent_driver
            .kill(&session.captain_handle)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message.clone()))?;
        self.agent_driver
            .kill(&session.mate_handle)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message.clone()))?;
        self.worktree_ops
            .remove_worktree(&session.worktree_path)
            .await
            .map_err(|error| SessionManagerError::Worktree(error.message))?;

        Ok(())
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
            emit_event(session, event);
        }

        self.persist_session(session_id).await?;
        Ok(())
    }

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

    async fn persist_session(&self, session_id: &SessionId) -> Result<(), SessionManagerError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;

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

fn emit_event(session: &mut ActiveSession, event: SessionEvent) {
    match &event {
        SessionEvent::AgentStateChanged { role, state } => match role {
            Role::Captain => session.captain.state = state.clone(),
            Role::Mate => session.mate.state = state.clone(),
        },
        SessionEvent::Content { role, block } => {
            if let Some(task) = session.current_task.as_mut() {
                task.content_history.push(TaskContentRecord {
                    role: *role,
                    block: block.clone(),
                });
            }
        }
        SessionEvent::PermissionRequested { role, request } => {
            session.pending_permissions.insert(
                request.permission_id.clone(),
                PendingPermission {
                    role: *role,
                    request: request.clone(),
                },
            );

            let state = AgentState::AwaitingPermission {
                request: request.clone(),
            };
            match role {
                Role::Captain => session.captain.state = state,
                Role::Mate => session.mate.state = state,
            }
        }
        SessionEvent::TaskStatusChanged { .. } => {}
        SessionEvent::ContextUpdated {
            role,
            remaining_percent,
        } => match role {
            Role::Captain => session.captain.context_remaining_percent = Some(*remaining_percent),
            Role::Mate => session.mate.context_remaining_percent = Some(*remaining_percent),
        },
    }

    let _ = session.events_tx.send(event);
}

fn transition_task(
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

    task.record.status = next;
    let task_id = task.record.id.clone();
    emit_event(
        session,
        SessionEvent::TaskStatusChanged {
            task_id,
            status: next,
        },
    );

    Ok(())
}

fn is_valid_transition(from: TaskStatus, to: TaskStatus) -> bool {
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

fn archive_terminal_task(session: &mut ActiveSession) {
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

fn current_task_status(session: &ActiveSession) -> Result<TaskStatus, SessionManagerError> {
    session
        .current_task
        .as_ref()
        .map(|task| task.record.status)
        .ok_or(SessionManagerError::NoActiveTask)
}

fn set_agent_state(session: &mut ActiveSession, role: Role, state: AgentState) {
    emit_event(session, SessionEvent::AgentStateChanged { role, state });
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
