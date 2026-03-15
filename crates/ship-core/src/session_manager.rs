use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use ship_types::{
    AgentAcpInfo, AgentSnapshot, AgentState, AutonomyMode, BlockId, BlockPatch,
    CloseSessionResponse, ContentBlock, CreateSessionRequest, CurrentTask, HumanReviewRequest,
    PermissionRequest, PermissionResolution, PersistedSession, Role, SessionConfig, SessionEvent,
    SessionEventEnvelope, SessionId, SessionStartupStage, SessionStartupState, SessionSummary,
    TaskContentRecord, TaskId, TaskRecord, TaskStatus, WorktreeDiffStats,
};
use tokio::sync::broadcast;

use crate::AgentSessionConfig;
use crate::{AgentDriver, SessionGitNames, SessionStore, StopReason, WorktreeOps};

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
    PermissionOptionNotFound(String),
    SessionStartupNotReady,
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
            Self::PermissionOptionNotFound(option_id) => {
                write!(f, "permission option not found: {option_id}")
            }
            Self::SessionStartupNotReady => f.write_str("session startup is not complete"),
        }
    }
}

impl std::error::Error for SessionManagerError {}

#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub block_id: BlockId,
    pub role: Role,
    pub request: PermissionRequest,
}

#[derive(Debug, Clone)]
pub struct PendingEdit {
    pub path: PathBuf,
    pub old_content: String,
    pub new_content: String,
    /// Original transformation parameters, kept so edit_confirm can re-apply
    /// the edit if the file was modified by a prior confirm in the same batch.
    pub old_string: String,
    pub new_string: String,
    pub replace_all: bool,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub id: SessionId,
    pub created_at: String,
    pub config: SessionConfig,
    pub worktree_path: Option<PathBuf>,
    pub captain_handle: Option<crate::AgentHandle>,
    pub mate_handle: Option<crate::AgentHandle>,
    pub captain: AgentSnapshot,
    pub mate: AgentSnapshot,
    pub startup_state: SessionStartupState,
    pub session_event_log: Vec<SessionEventEnvelope>,
    pub current_task: Option<CurrentTask>,
    pub task_history: Vec<TaskRecord>,
    pub diff_stats: Option<WorktreeDiffStats>,
    pub captain_block_count: usize,
    pub mate_block_count: usize,
    pub pending_permissions: HashMap<String, PendingPermission>,
    pub pending_edits: HashMap<String, PendingEdit>,
    pub pending_steer: Option<String>,
    pub pending_human_review: Option<HumanReviewRequest>,
    pub title: Option<String>,
    pub archived_at: Option<String>,
    pub captain_acp_session_id: Option<String>,
    pub mate_acp_session_id: Option<String>,
    pub captain_acp_info: Option<AgentAcpInfo>,
    pub mate_acp_info: Option<AgentAcpInfo>,
    pub events_tx: broadcast::Sender<SessionEventEnvelope>,
    pub next_event_seq: u64,
    pub captain_prompt_gen: u64,
    pub mate_prompt_gen: u64,
    pub is_read: bool,
    pub captain_has_ever_assigned: bool,
    pub captain_delegation_reminded: bool,
    // Runtime-only fields (not persisted)
    pub snapshot_manager: Option<ship_code::snapshot::SnapshotManager>,
    pub utility_handle: Option<crate::AgentHandle>,
    pub utility_last_task_id: Option<TaskId>,
    pub mate_activity_buffer: Vec<String>,
    pub mate_activity_first_at: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
pub struct SessionStateView {
    pub id: SessionId,
    pub config: SessionConfig,
    pub captain: AgentSnapshot,
    pub mate: AgentSnapshot,
    pub startup_state: SessionStartupState,
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
        _repo_root: &Path,
    ) -> Result<SessionId, SessionManagerError> {
        let session_id = SessionId::new();
        let session_git_names = SessionGitNames::from_session_id(&session_id);
        let mcp_servers = req.mcp_servers.clone().unwrap_or_default();
        let (events_tx, _) = broadcast::channel(256);

        let session = ActiveSession {
            id: session_id.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            config: SessionConfig {
                project: req.project,
                base_branch: req.base_branch,
                branch_name: session_git_names.branch_name,
                captain_kind: req.captain_kind,
                mate_kind: req.mate_kind,
                captain_preset_id: req.captain_preset_id.clone(),
                mate_preset_id: req.mate_preset_id.clone(),
                captain_provider: Some(req.captain_kind.default_provider_id()),
                mate_provider: Some(req.mate_kind.default_provider_id()),
                captain_model_id: None,
                mate_model_id: None,
                autonomy_mode: AutonomyMode::HumanInTheLoop,
                mcp_servers,
            },
            worktree_path: None,
            captain_handle: None,
            mate_handle: None,
            captain: AgentSnapshot {
                role: Role::Captain,
                kind: req.captain_kind,
                state: AgentState::Idle,
                context_remaining_percent: None,
                preset_id: req.captain_preset_id.clone(),
                provider: Some(req.captain_kind.default_provider_id()),
                model_id: None,
                available_models: Vec::new(),
                effort_config_id: None,
                effort_value_id: None,
                available_effort_values: Vec::new(),
            },
            mate: AgentSnapshot {
                role: Role::Mate,
                kind: req.mate_kind,
                state: AgentState::Idle,
                context_remaining_percent: None,
                preset_id: req.mate_preset_id.clone(),
                provider: Some(req.mate_kind.default_provider_id()),
                model_id: None,
                available_models: Vec::new(),
                effort_config_id: None,
                effort_value_id: None,
                available_effort_values: Vec::new(),
            },
            startup_state: SessionStartupState::Pending,
            session_event_log: Vec::new(),
            current_task: None,
            task_history: Vec::new(),
            diff_stats: None,
            captain_block_count: 0,
            mate_block_count: 0,
            pending_permissions: HashMap::new(),
            pending_edits: HashMap::new(),
            pending_steer: None,
            pending_human_review: None,
            title: None,
            archived_at: None,
            captain_acp_session_id: None,
            mate_acp_session_id: None,
            captain_acp_info: None,
            mate_acp_info: None,
            events_tx,
            next_event_seq: 0,
            captain_prompt_gen: 0,
            mate_prompt_gen: 0,
            is_read: true,
            captain_has_ever_assigned: false,
            captain_delegation_reminded: false,
            snapshot_manager: None,
            utility_handle: None,
            utility_last_task_id: None,
            mate_activity_buffer: Vec::new(),
            mate_activity_first_at: None,
        };

        self.sessions.insert(session_id.clone(), session);
        self.persist_session(&session_id).await?;
        Ok(session_id)
    }

    pub async fn start_session(
        &mut self,
        session_id: &SessionId,
        repo_root: &Path,
    ) -> Result<(), SessionManagerError> {
        self.set_startup_state(
            session_id,
            SessionStartupState::Running {
                stage: SessionStartupStage::CreatingWorktree,
                message: "Creating worktree".to_owned(),
            },
        )
        .await?;

        let session_git_names = SessionGitNames::from_session_id(session_id);
        let (base_branch, captain_kind, mate_kind, mcp_servers, captain_acp_id, mate_acp_id) = {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            (
                session.config.base_branch.clone(),
                session.config.captain_kind,
                session.config.mate_kind,
                session.config.mcp_servers.clone(),
                session.captain_acp_session_id.clone(),
                session.mate_acp_session_id.clone(),
            )
        };

        let worktree_path = self
            .worktree_ops
            .create_worktree(
                &session_git_names.branch_name,
                &session_git_names.worktree_dir,
                &base_branch,
                repo_root,
            )
            .await
            .map_err(|error| SessionManagerError::Worktree(error.message))?;

        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            session.worktree_path = Some(worktree_path.clone());
            session.config.branch_name = session_git_names.branch_name.clone();
        }
        self.persist_session(session_id).await?;

        let captain_config = AgentSessionConfig {
            worktree_path: worktree_path.clone(),
            mcp_servers: mcp_servers.clone(),
            resume_session_id: captain_acp_id,
        };
        let mate_config = AgentSessionConfig {
            worktree_path: worktree_path.clone(),
            mcp_servers: mcp_servers.clone(),
            resume_session_id: mate_acp_id,
        };

        self.set_startup_state(
            session_id,
            SessionStartupState::Running {
                stage: SessionStartupStage::StartingCaptain,
                message: "Starting captain".to_owned(),
            },
        )
        .await?;
        let captain_spawn = self
            .agent_driver
            .spawn(captain_kind, Role::Captain, &captain_config)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;
        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            session.captain_handle = Some(captain_spawn.handle);
            session.captain_acp_session_id = Some(captain_spawn.acp_session_id);
            if captain_spawn.model_id.is_some() {
                session.captain.model_id = captain_spawn.model_id;
            }
            if !captain_spawn.available_models.is_empty() {
                session.captain.available_models = captain_spawn.available_models;
            }
            if captain_spawn.effort_config_id.is_some() {
                session.captain.effort_config_id = captain_spawn.effort_config_id;
            }
            if captain_spawn.effort_value_id.is_some() {
                session.captain.effort_value_id = captain_spawn.effort_value_id;
            }
            if !captain_spawn.available_effort_values.is_empty() {
                session.captain.available_effort_values = captain_spawn.available_effort_values;
            }
        }
        self.persist_session(session_id).await?;

        self.set_startup_state(
            session_id,
            SessionStartupState::Running {
                stage: SessionStartupStage::StartingMate,
                message: "Starting mate".to_owned(),
            },
        )
        .await?;
        let mate_spawn = self
            .agent_driver
            .spawn(mate_kind, Role::Mate, &mate_config)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;
        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            session.mate_handle = Some(mate_spawn.handle);
            session.mate_acp_session_id = Some(mate_spawn.acp_session_id);
            if mate_spawn.model_id.is_some() {
                session.mate.model_id = mate_spawn.model_id;
            }
            if !mate_spawn.available_models.is_empty() {
                session.mate.available_models = mate_spawn.available_models;
            }
            if mate_spawn.effort_config_id.is_some() {
                session.mate.effort_config_id = mate_spawn.effort_config_id;
            }
            if mate_spawn.effort_value_id.is_some() {
                session.mate.effort_value_id = mate_spawn.effort_value_id;
            }
            if !mate_spawn.available_effort_values.is_empty() {
                session.mate.available_effort_values = mate_spawn.available_effort_values;
            }
        }
        self.set_startup_state(session_id, SessionStartupState::Ready)
            .await
    }

    async fn set_startup_state(
        &mut self,
        session_id: &SessionId,
        state: SessionStartupState,
    ) -> Result<(), SessionManagerError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
        apply_event(
            session,
            SessionEvent::SessionStartupChanged {
                state: state.clone(),
            },
        );
        self.persist_session(session_id).await
    }

    pub async fn set_agent_model(
        &mut self,
        session_id: &SessionId,
        role: Role,
        model_id: String,
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
            .ok_or(SessionManagerError::Agent("agent not spawned".to_owned()))?
        };
        self.agent_driver
            .set_model(&handle, &model_id)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
        let available_models = match role {
            Role::Captain => session.captain.available_models.clone(),
            Role::Mate => session.mate.available_models.clone(),
        };
        apply_event(
            session,
            SessionEvent::AgentModelChanged {
                role,
                model_id: Some(model_id),
                available_models,
            },
        );
        self.persist_session(session_id).await
    }

    pub async fn set_agent_effort(
        &mut self,
        session_id: &SessionId,
        role: Role,
        config_id: String,
        value_id: String,
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
            .ok_or(SessionManagerError::Agent("agent not spawned".to_owned()))?
        };
        self.agent_driver
            .set_effort(&handle, &config_id, &value_id)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
        let available_effort_values = match role {
            Role::Captain => session.captain.available_effort_values.clone(),
            Role::Mate => session.mate.available_effort_values.clone(),
        };
        apply_event(
            session,
            SessionEvent::AgentEffortChanged {
                role,
                effort_config_id: Some(config_id),
                effort_value_id: Some(value_id),
                available_effort_values,
            },
        );
        self.persist_session(session_id).await
    }

    // r[proto.list-sessions]
    pub fn list_sessions(&self) -> Vec<SessionSummary> {
        self.sessions
            .values()
            .map(|session| {
                let tasks_done = session
                    .task_history
                    .iter()
                    .filter(|task| task.status == TaskStatus::Accepted)
                    .count() as u32;
                let tasks_total =
                    session.task_history.len() as u32 + u32::from(session.current_task.is_some());

                SessionSummary {
                    id: session.id.clone(),
                    slug: SessionGitNames::from_session_id(&session.id).slug,
                    project: session.config.project.clone(),
                    branch_name: session.config.branch_name.clone(),
                    title: session.title.clone(),
                    captain: session.captain.clone(),
                    mate: session.mate.clone(),
                    startup_state: session.startup_state.clone(),
                    current_task_title: session
                        .current_task
                        .as_ref()
                        .map(|task| task.record.title.clone()),
                    current_task_description: session
                        .current_task
                        .as_ref()
                        .map(|task| task.record.description.clone()),
                    task_status: session.current_task.as_ref().map(|task| task.record.status),
                    diff_stats: session.diff_stats.clone(),
                    tasks_done,
                    tasks_total,
                    autonomy_mode: session.config.autonomy_mode,
                    created_at: session.created_at.clone(),
                    is_admiral: false,
                    is_read: session.is_read,
                }
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
            startup_state: session.startup_state.clone(),
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

        let mut live_rx = session.events_tx.subscribe();
        let raw_replay: Vec<SessionEventEnvelope> = session
            .current_task
            .as_ref()
            .map(|task| task.event_log.clone())
            .unwrap_or_default();
        let replay = coalesce_replay_events(&raw_replay);
        let (subscriber_tx, subscriber_rx) = broadcast::channel(256);

        for envelope in replay {
            // BlockFinalized is internal-only — don't replay to subscribers.
            if matches!(envelope.event, SessionEvent::BlockFinalized { .. }) {
                continue;
            }
            let _ = subscriber_tx.send(envelope);
        }

        tokio::spawn(async move {
            loop {
                match live_rx.recv().await {
                    Ok(envelope) => {
                        // BlockFinalized is internal-only — don't forward to subscribers.
                        if matches!(envelope.event, SessionEvent::BlockFinalized { .. }) {
                            continue;
                        }
                        if subscriber_tx.send(envelope).is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(subscriber_rx)
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

    // r[captain.tool.assign]
    pub async fn assign(
        &mut self,
        session_id: &SessionId,
        title: String,
        description: String,
    ) -> Result<TaskId, SessionManagerError> {
        {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            if session.startup_state != SessionStartupState::Ready {
                return Err(SessionManagerError::SessionStartupNotReady);
            }
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
                    title: title.clone(),
                    description: description.clone(),
                    status: TaskStatus::Assigned,
                    steps: Vec::new(),
                    assigned_at: Some(chrono::Utc::now().to_rfc3339()),
                    completed_at: None,
                },
                pending_mate_guidance: None,
                content_history: Vec::new(),
                event_log: Vec::new(),
            });

            apply_event(
                session,
                SessionEvent::TaskStarted {
                    task_id: task_id.clone(),
                    title: title.clone(),
                    description: description.clone(),
                    steps: Vec::new(),
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
            if status != TaskStatus::Assigned
                && status != TaskStatus::ReviewPending
                && status != TaskStatus::SteerPending
            {
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
                        source: ship_types::TextSource::AgentMessage,
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

            self.prompt_mate(
                session_id,
                format!("@mate {message}\n\nAct on this correction and continue working."),
            )
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
            if status != TaskStatus::Assigned
                && status != TaskStatus::ReviewPending
                && status != TaskStatus::SteerPending
            {
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
        let (status, handle) = {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            let status = current_task_status(session)?;
            if status.is_terminal() {
                return Err(SessionManagerError::NoActiveTask);
            }
            (status, session.mate_handle.clone())
        };

        if status == TaskStatus::Working
            && let Some(handle) = handle
        {
            self.agent_driver
                .cancel(&handle)
                .await
                .map_err(|error| SessionManagerError::Agent(error.message))?;
        }

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
        option_id: &str,
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
            }
            .ok_or_else(|| SessionManagerError::Agent("agent not ready".to_owned()))?;

            (pending, handle)
        };

        self.agent_driver
            .resolve_permission(&handle, permission_id, option_id)
            .await
            .map_err(|error| SessionManagerError::Agent(error.message))?;

        let resolution = pending
            .request
            .options
            .as_ref()
            .and_then(|options| options.iter().find(|option| option.option_id == option_id))
            .map(|option| match option.kind {
                ship_types::PermissionOptionKind::AllowOnce
                | ship_types::PermissionOptionKind::AllowAlways => PermissionResolution::Approved,
                ship_types::PermissionOptionKind::RejectOnce
                | ship_types::PermissionOptionKind::RejectAlways
                | ship_types::PermissionOptionKind::Other => PermissionResolution::Denied,
            })
            .ok_or_else(|| SessionManagerError::PermissionOptionNotFound(option_id.to_owned()))?;

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
                patch: BlockPatch::PermissionResolve { resolution },
            },
        );

        Ok(())
    }

    fn repo_root_for_worktree(worktree_path: &Path) -> Result<&Path, SessionManagerError> {
        let mut current = worktree_path;
        loop {
            let parent = current.parent().ok_or_else(|| {
                SessionManagerError::Worktree(format!(
                    "invalid worktree path: {}",
                    worktree_path.display()
                ))
            })?;
            if parent.file_name().and_then(|n| n.to_str()) == Some(".ship") {
                return parent.parent().ok_or_else(|| {
                    SessionManagerError::Worktree(format!(
                        "invalid worktree path: {}",
                        worktree_path.display()
                    ))
                });
            }
            current = parent;
            if worktree_path
                .components()
                .count()
                .saturating_sub(current.components().count())
                > 3
            {
                return Err(SessionManagerError::Worktree(format!(
                    "invalid worktree path: {}",
                    worktree_path.display()
                )));
            }
        }
    }

    async fn cleanup_session_resources(
        &self,
        session: &ActiveSession,
        force: bool,
    ) -> Result<(), SessionManagerError> {
        if let Some(handle) = &session.captain_handle {
            self.agent_driver
                .kill(handle)
                .await
                .map_err(|error| SessionManagerError::Agent(error.message.clone()))?;
        }
        if let Some(handle) = &session.mate_handle {
            self.agent_driver
                .kill(handle)
                .await
                .map_err(|error| SessionManagerError::Agent(error.message.clone()))?;
        }

        let Some(worktree_path) = session.worktree_path.as_ref() else {
            return Ok(());
        };
        let repo_root = Self::repo_root_for_worktree(worktree_path)?;

        self.worktree_ops
            .remove_worktree(worktree_path, force)
            .await
            .map_err(|error| SessionManagerError::Worktree(error.message))?;

        let branch_exists = self
            .worktree_ops
            .list_branches(repo_root)
            .await
            .map_err(|error| SessionManagerError::Worktree(error.message))?
            .iter()
            .any(|branch| branch == &session.config.branch_name);

        if branch_exists {
            self.worktree_ops
                .delete_branch(&session.config.branch_name, force, repo_root)
                .await
                .map_err(|error| SessionManagerError::Worktree(error.message))?;
        }

        Ok(())
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
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?
            .clone();
        let worktree_path = session.worktree_path.clone();

        if let Some(worktree_path) = worktree_path
            && self
                .worktree_ops
                .has_uncommitted_changes(&worktree_path)
                .await
                .map_err(|error| SessionManagerError::Worktree(error.message))?
            && !force
        {
            return Ok(CloseSessionResponse::RequiresConfirmation);
        }

        self.cleanup_session_resources(&session, force).await?;
        self.store
            .delete_session(session_id)
            .await
            .map_err(|error| SessionManagerError::Store(error.message))?;
        self.sessions.remove(session_id);

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
            .ok_or_else(|| SessionManagerError::Agent("agent not ready".to_owned()))?
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

        // Update last_event_at in the AcpInfo for the drained role and broadcast the change.
        let now = chrono::Utc::now().to_rfc3339();
        let updated_info = {
            let session = self
                .sessions
                .get(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            match role {
                Role::Captain => session.captain_acp_info.as_ref().map(|i| {
                    let mut i = i.clone();
                    i.last_event_at = Some(now.clone());
                    i
                }),
                Role::Mate => session.mate_acp_info.as_ref().map(|i| {
                    let mut i = i.clone();
                    i.last_event_at = Some(now);
                    i
                }),
            }
        };
        if let Some(info) = updated_info {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
            apply_event(session, SessionEvent::AgentAcpInfoChanged { role, info });
        }

        self.persist_session(session_id).await?;
        Ok(())
    }

    async fn prompt_mate(
        &mut self,
        session_id: &SessionId,
        prompt: String,
    ) -> Result<(), SessionManagerError> {
        let stop_reason = self.prompt_agent(session_id, Role::Mate, prompt).await?;

        match stop_reason {
            StopReason::EndTurn => {
                let session = self
                    .sessions
                    .get_mut(session_id)
                    .ok_or_else(|| SessionManagerError::SessionNotFound(session_id.clone()))?;
                transition_task(session, TaskStatus::ReviewPending)?;
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
            .ok_or_else(|| SessionManagerError::Agent("agent not ready".to_owned()))?
        };

        let parts = vec![ship_types::PromptContentPart::Text { text: prompt }];
        let response = self
            .agent_driver
            .prompt(&handle, &parts)
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
            created_at: session.created_at.clone(),
            config: session.config.clone(),
            captain: session.captain.clone(),
            mate: session.mate.clone(),
            startup_state: session.startup_state.clone(),
            session_event_log: session.session_event_log.clone(),
            current_task: session.current_task.clone(),
            task_history: session.task_history.clone(),
            title: session.title.clone(),
            archived_at: session.archived_at.clone(),
            captain_acp_session_id: session.captain_acp_session_id.clone(),
            mate_acp_session_id: session.mate_acp_session_id.clone(),
            is_read: session.is_read,
            captain_has_ever_assigned: session.captain_has_ever_assigned,
            captain_delegation_reminded: session.captain_delegation_reminded,
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
    // Preserve existing plan when transitioning to Working with plan: None.
    // Activity-only transitions (permission resolved, prompt in progress) should not
    // clear a plan that was previously set via plan_create or plan_step_complete.
    let event = match event {
        SessionEvent::AgentStateChanged {
            role,
            state:
                AgentState::Working {
                    plan: None,
                    activity,
                },
        } => {
            let existing_plan = match role {
                Role::Captain => {
                    if let AgentState::Working { plan, .. } = &session.captain.state {
                        plan.clone()
                    } else {
                        None
                    }
                }
                Role::Mate => {
                    if let AgentState::Working { plan, .. } = &session.mate.state {
                        plan.clone()
                    } else {
                        None
                    }
                }
            };
            SessionEvent::AgentStateChanged {
                role,
                state: AgentState::Working {
                    plan: existing_plan,
                    activity,
                },
            }
        }
        other => other,
    };

    if let SessionEvent::AgentStateChanged {
        role,
        state: AgentState::Working {
            plan: Some(steps), ..
        },
    } = &event
    {
        sync_plan_block(session, *role, steps.clone());
    }

    let envelope = SessionEventEnvelope {
        seq: session.next_event_seq,
        timestamp: chrono::Utc::now().to_rfc3339(),
        event,
    };
    session.next_event_seq = session.next_event_seq.saturating_add(1);

    if event_is_session_scoped(session, &envelope.event) {
        session.session_event_log.push(envelope.clone());
    } else if let Some(task) = session.current_task.as_mut() {
        task.event_log.push(envelope.clone());
    }

    apply_event_to_materialized_state(session, &envelope.event);
    let _ = session.events_tx.send(envelope);
}

fn event_is_session_scoped(session: &ActiveSession, event: &SessionEvent) -> bool {
    session.current_task.is_none() || matches!(event, SessionEvent::SessionStartupChanged { .. })
}

fn sync_plan_block(session: &mut ActiveSession, role: Role, steps: Vec<ship_types::PlanStep>) {
    if let Some(block_id) = find_plan_block_id(session, role) {
        apply_event(
            session,
            SessionEvent::BlockPatch {
                // r[event.block-id.plan]
                block_id,
                role,
                // r[event.patch.plan-replace]
                patch: BlockPatch::PlanReplace { steps },
            },
        );
        return;
    }

    apply_event(
        session,
        SessionEvent::BlockAppend {
            // r[event.block-id.plan]
            block_id: BlockId::new(),
            role,
            block: ContentBlock::PlanUpdate { steps },
        },
    );
}

fn find_plan_block_id(session: &ActiveSession, role: Role) -> Option<BlockId> {
    session
        .current_task
        .as_ref()?
        .content_history
        .iter()
        .rev()
        .find(|record| {
            record.role == role && matches!(record.block, ContentBlock::PlanUpdate { .. })
        })
        .map(|record| record.block_id.clone())
}

// r[backend.materialized-state]
pub fn apply_event_to_materialized_state(session: &mut ActiveSession, event: &SessionEvent) {
    match &event {
        SessionEvent::BlockAppend {
            block_id,
            role,
            block,
        } => {
            if *role == Role::Mate
                && let Some(task) = session.current_task.as_mut()
                && let ContentBlock::PlanUpdate { steps } = block
            {
                task.record.steps = steps.clone();
            }
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
                permission_id,
                tool_call_id,
                tool_name,
                arguments,
                description,
                kind,
                target,
                raw_input,
                options,
                ..
            } = block
            {
                let permission_id = permission_id
                    .clone()
                    .unwrap_or_else(|| block_id.0.to_string());
                let request = PermissionRequest {
                    permission_id: permission_id.clone(),
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    arguments: arguments.clone(),
                    description: description.clone(),
                    kind: *kind,
                    target: target.clone(),
                    raw_input: raw_input.clone(),
                    options: options.clone(),
                };
                session.pending_permissions.insert(
                    permission_id.clone(),
                    PendingPermission {
                        block_id: block_id.clone(),
                        role: *role,
                        request: request.clone(),
                    },
                );

                let state = AgentState::AwaitingPermission {
                    request: Box::new(request),
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
            if *role == Role::Mate
                && let Some(task) = session.current_task.as_mut()
                && let BlockPatch::PlanReplace { steps } = patch
            {
                task.record.steps = steps.clone();
            }
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
            Role::Mate => {
                session.mate.state = state.clone();
                if let AgentState::Working {
                    plan: Some(steps), ..
                } = state
                    && let Some(task) = session.current_task.as_mut()
                {
                    task.record.steps = steps.clone();
                }
            }
        },
        SessionEvent::SessionStartupChanged { state } => {
            session.startup_state = state.clone();
        }
        SessionEvent::TaskStatusChanged { task_id, status } => {
            if let Some(task) = session.current_task.as_mut()
                && task.record.id == *task_id
            {
                task.record.status = *status;
                if status.is_terminal() && task.record.completed_at.is_none() {
                    task.record.completed_at = Some(chrono::Utc::now().to_rfc3339());
                }
            }
        }
        SessionEvent::TaskStarted {
            task_id,
            title,
            description,
            steps,
        } => {
            if let Some(task) = session.current_task.as_mut() {
                task.record.id = task_id.clone();
                task.record.title = title.clone();
                task.record.description = description.clone();
                task.record.steps = steps.clone();
            }
        }
        SessionEvent::ContextUpdated {
            role,
            remaining_percent,
        } => match role {
            Role::Captain => session.captain.context_remaining_percent = Some(*remaining_percent),
            Role::Mate => session.mate.context_remaining_percent = Some(*remaining_percent),
        },
        SessionEvent::AgentModelChanged {
            role,
            model_id,
            available_models,
        } => match role {
            Role::Captain => {
                session.captain.model_id = model_id.clone();
                session.captain.available_models = available_models.clone();
            }
            Role::Mate => {
                session.mate.model_id = model_id.clone();
                session.mate.available_models = available_models.clone();
            }
        },
        SessionEvent::AgentPresetChanged {
            role,
            preset_id,
            kind,
            provider,
        } => match role {
            Role::Captain => {
                session.captain.preset_id = preset_id.clone();
                session.captain.kind = *kind;
                session.captain.provider = provider.clone();
            }
            Role::Mate => {
                session.mate.preset_id = preset_id.clone();
                session.mate.kind = *kind;
                session.mate.provider = provider.clone();
            }
        },
        SessionEvent::AgentEffortChanged {
            role,
            effort_config_id,
            effort_value_id,
            available_effort_values,
        } => match role {
            Role::Captain => {
                session.captain.effort_config_id = effort_config_id.clone();
                session.captain.effort_value_id = effort_value_id.clone();
                if !available_effort_values.is_empty() {
                    session.captain.available_effort_values = available_effort_values.clone();
                }
            }
            Role::Mate => {
                session.mate.effort_config_id = effort_config_id.clone();
                session.mate.effort_value_id = effort_value_id.clone();
                if !available_effort_values.is_empty() {
                    session.mate.available_effort_values = available_effort_values.clone();
                }
            }
        },
        // Handled by the session manager before apply_event is called;
        // should never reach here.
        SessionEvent::MateGuidanceQueued { .. } => {}
        SessionEvent::HumanReviewRequested {
            message,
            diff,
            worktree_path,
        } => {
            session.pending_human_review = Some(HumanReviewRequest {
                message: message.clone(),
                diff: diff.clone(),
                worktree_path: worktree_path.clone(),
            });
        }
        SessionEvent::HumanReviewCleared => {
            session.pending_human_review = None;
        }
        // r[event.session-title-changed]
        SessionEvent::SessionTitleChanged { title } => {
            session.title = Some(title.clone());
        }
        // r[acp.debug-info]
        SessionEvent::AgentAcpInfoChanged { role, info } => match role {
            Role::Captain => session.captain_acp_info = Some(info.clone()),
            Role::Mate => session.mate_acp_info = Some(info.clone()),
        },
        // Checks lifecycle events carry no materialized state — subscribers
        // observe them via the live broadcast channel only.
        SessionEvent::ChecksStarted { .. } | SessionEvent::ChecksFinished { .. } => {}
        // BlockFinalized is an internal event for server-side processing
        // (e.g. mention detection). No materialized state to update.
        SessionEvent::BlockFinalized { .. } => {}
    }
}

pub fn apply_block_patch(block: &mut ContentBlock, patch: &BlockPatch) {
    match patch {
        BlockPatch::TextAppend { text } => {
            if let ContentBlock::Text { text: existing, .. } = block {
                existing.push_str(text);
            }
        }
        BlockPatch::ToolCallUpdate {
            tool_name,
            kind,
            target,
            raw_input,
            raw_output,
            status,
            locations,
            content,
            error,
        } => {
            if let ContentBlock::ToolCall {
                tool_name: existing_tool_name,
                kind: existing_kind,
                target: existing_target,
                raw_input: existing_raw_input,
                raw_output: existing_raw_output,
                locations: existing_locations,
                status: existing_status,
                content: existing_content,
                error: existing_error,
                ..
            } = block
            {
                if let Some(tool_name) = tool_name {
                    *existing_tool_name = tool_name.clone();
                }
                if let Some(kind) = kind {
                    *existing_kind = Some(*kind);
                }
                if let Some(target) = target.as_ref() {
                    *existing_target = Some(target.clone());
                }
                if let Some(raw_input) = raw_input {
                    *existing_raw_input = Some(raw_input.clone());
                }
                if let Some(raw_output) = raw_output {
                    *existing_raw_output = Some(raw_output.clone());
                }
                *existing_status = *status;
                if let Some(locations) = locations {
                    *existing_locations = locations.clone();
                }
                if let Some(content) = content {
                    *existing_content = content.clone();
                }
                if let Some(error) = error {
                    *existing_error = Some(error.clone());
                }
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
    if let Some(current_task) = session.current_task.as_mut() {
        current_task.content_history.clear();
    }
    session.pending_permissions.clear();
    session.captain_block_count = 0;
    session.mate_block_count = 0;
    session.captain.state = AgentState::Idle;
    session.mate.state = AgentState::Idle;
    session.captain.context_remaining_percent = None;
    session.mate.context_remaining_percent = None;

    let replay = session
        .session_event_log
        .iter()
        .cloned()
        .chain(
            session
                .current_task
                .as_ref()
                .into_iter()
                .flat_map(|task| task.event_log.clone()),
        )
        .collect::<Vec<_>>();
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
            | (TaskStatus::Assigned, TaskStatus::SteerPending)
            | (TaskStatus::Assigned, TaskStatus::Accepted)
            | (TaskStatus::Working, TaskStatus::ReviewPending)
            | (TaskStatus::ReviewPending, TaskStatus::SteerPending)
            | (TaskStatus::ReviewPending, TaskStatus::Working)
            | (TaskStatus::ReviewPending, TaskStatus::Accepted)
            | (TaskStatus::SteerPending, TaskStatus::Working)
            | (TaskStatus::SteerPending, TaskStatus::Accepted)
            | (TaskStatus::Assigned, TaskStatus::RebaseConflict)
            | (TaskStatus::ReviewPending, TaskStatus::RebaseConflict)
            | (TaskStatus::SteerPending, TaskStatus::RebaseConflict)
            | (TaskStatus::RebaseConflict, TaskStatus::ReviewPending)
            | (TaskStatus::RebaseConflict, TaskStatus::Accepted)
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
        // Merge the task's event log into the session log so that replay
        // chains (session_event_log + current_task.event_log) stay gapless
        // across task boundaries.
        session.session_event_log.extend(task.event_log);
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

/// Fold `BlockAppend` + subsequent `BlockPatch` sequences into a single
/// `BlockAppend` carrying the fully-materialized block state. Non-block events
/// pass through unchanged. The original sequence numbers and timestamps are
/// preserved for each retained event.
///
/// This is applied to replay events only. Live events always flow individually.
// r[event.subscribe.replay]
pub fn coalesce_replay_events(events: &[SessionEventEnvelope]) -> Vec<SessionEventEnvelope> {
    // Pass 1: build the final materialized state for every block.
    let mut final_states: HashMap<BlockId, (Role, ContentBlock)> = HashMap::new();
    for envelope in events {
        match &envelope.event {
            SessionEvent::BlockAppend {
                block_id,
                role,
                block,
            } => {
                final_states.insert(block_id.clone(), (*role, block.clone()));
            }
            SessionEvent::BlockPatch {
                block_id, patch, ..
            } => {
                if let Some((_, block)) = final_states.get_mut(block_id) {
                    apply_block_patch(block, patch);
                }
            }
            _ => {}
        }
    }

    // Pass 2: emit one coalesced BlockAppend per block; skip all BlockPatch events.
    let mut result = Vec::with_capacity(events.len());
    let mut emitted: HashSet<BlockId> = HashSet::new();
    for envelope in events {
        match &envelope.event {
            SessionEvent::BlockAppend { block_id, .. } => {
                if let Some((role, block)) = final_states.get(block_id) {
                    result.push(SessionEventEnvelope {
                        seq: envelope.seq,
                        timestamp: envelope.timestamp.clone(),
                        event: SessionEvent::BlockAppend {
                            block_id: block_id.clone(),
                            role: *role,
                            block: block.clone(),
                        },
                    });
                    emitted.insert(block_id.clone());
                }
            }
            SessionEvent::BlockPatch { block_id, .. } => {
                // Skip — already folded into the preceding BlockAppend.
                // If there's somehow a patch without a prior append, pass through.
                if !emitted.contains(block_id) {
                    result.push(envelope.clone());
                }
            }
            _ => result.push(envelope.clone()),
        }
    }

    result
}
