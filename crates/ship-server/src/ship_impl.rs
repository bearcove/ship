use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use roam::Tx;
use ship_core::{
    AcpAgentDriver, ActiveSession, AgentDriver, GitWorktreeOps, JsonSessionStore, ProjectRegistry,
    SessionStore, WorktreeOps, apply_event, archive_terminal_task, current_task_status,
    rebuild_materialized_from_event_log, set_agent_state, transition_task,
};
use ship_service::Ship;
use ship_types::{
    AgentKind, AgentSnapshot, AgentState, AutonomyMode, BlockId, CloseSessionRequest,
    CloseSessionResponse, ContentBlock, CreateSessionRequest, CreateSessionResponse, CurrentTask,
    PersistedSession, ProjectInfo, ProjectName, Role, SessionConfig, SessionDetail, SessionEvent,
    SessionId, SessionSummary, SubscribeMessage, TaskId, TaskRecord, TaskStatus,
};
use tokio::sync::broadcast;

// r[server.multi-repo]
#[derive(Clone)]
pub struct ShipImpl {
    registry: Arc<tokio::sync::Mutex<ProjectRegistry>>,
    agent_driver: Arc<AcpAgentDriver>,
    worktree_ops: Arc<GitWorktreeOps>,
    store: Arc<JsonSessionStore>,
    sessions: Arc<Mutex<HashMap<SessionId, ActiveSession>>>,
    repo_root: Arc<std::path::PathBuf>,
}

impl ShipImpl {
    pub fn new(
        registry: ProjectRegistry,
        sessions_dir: std::path::PathBuf,
        repo_root: std::path::PathBuf,
    ) -> Self {
        Self {
            registry: Arc::new(tokio::sync::Mutex::new(registry)),
            agent_driver: Arc::new(AcpAgentDriver::new()),
            worktree_ops: Arc::new(GitWorktreeOps),
            store: Arc::new(JsonSessionStore::new(sessions_dir)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            repo_root: Arc::new(repo_root),
        }
    }

    fn fallback_agent(role: Role, kind: AgentKind) -> AgentSnapshot {
        AgentSnapshot {
            role,
            kind,
            state: AgentState::Error {
                message: "session not found".to_owned(),
            },
            context_remaining_percent: None,
        }
    }

    fn to_session_summary(session: &ActiveSession) -> SessionSummary {
        SessionSummary {
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
        }
    }

    fn to_session_detail(session: &ActiveSession) -> SessionDetail {
        SessionDetail {
            id: session.id.clone(),
            project: session.config.project.clone(),
            branch_name: session.config.branch_name.clone(),
            captain: session.captain.clone(),
            mate: session.mate.clone(),
            current_task: session
                .current_task
                .as_ref()
                .map(|task| task.record.clone()),
            task_history: session.task_history.clone(),
            autonomy_mode: session.config.autonomy_mode,
            pending_steer: session.pending_steer.clone(),
        }
    }

    fn fallback_session_detail(id: SessionId) -> SessionDetail {
        SessionDetail {
            id,
            project: ProjectName("unknown".to_owned()),
            branch_name: String::new(),
            captain: Self::fallback_agent(Role::Captain, AgentKind::Claude),
            mate: Self::fallback_agent(Role::Mate, AgentKind::Codex),
            current_task: None,
            task_history: Vec::new(),
            autonomy_mode: AutonomyMode::HumanInTheLoop,
            pending_steer: None,
        }
    }

    fn log_error(action: &str, error: &str) {
        tracing::warn!(%action, %error, "ship_impl call failed");
    }

    fn repo_root_for_worktree(worktree_path: &std::path::Path) -> Result<&std::path::Path, String> {
        let worktrees_dir = worktree_path
            .parent()
            .ok_or_else(|| format!("invalid worktree path: {}", worktree_path.display()))?;
        let ship_dir = worktrees_dir
            .parent()
            .ok_or_else(|| format!("invalid worktree path: {}", worktree_path.display()))?;
        let repo_root = ship_dir
            .parent()
            .ok_or_else(|| format!("invalid worktree path: {}", worktree_path.display()))?;

        if worktrees_dir.file_name().and_then(|name| name.to_str()) != Some("worktrees")
            || ship_dir.file_name().and_then(|name| name.to_str()) != Some(".ship")
        {
            return Err(format!(
                "invalid worktree path: {}",
                worktree_path.display()
            ));
        }

        Ok(repo_root)
    }

    async fn cleanup_session_resources(
        &self,
        session: &ActiveSession,
        force: bool,
    ) -> Result<(), String> {
        let repo_root = Self::repo_root_for_worktree(&session.worktree_path)?;

        if let Err(error) = self.agent_driver.kill(&session.captain_handle).await {
            Self::log_error("close_session_kill_captain", &error.message);
        }
        if let Err(error) = self.agent_driver.kill(&session.mate_handle).await {
            Self::log_error("close_session_kill_mate", &error.message);
        }

        if session.worktree_path.exists() {
            self.worktree_ops
                .remove_worktree(&session.worktree_path, force)
                .await
                .map_err(|error| error.message)?;
        }

        let branch_exists = self
            .worktree_ops
            .list_branches(repo_root)
            .await
            .map_err(|error| error.message)?
            .iter()
            .any(|branch| branch == &session.config.branch_name);

        if branch_exists {
            self.worktree_ops
                .delete_branch(&session.config.branch_name, force, repo_root)
                .await
                .map_err(|error| error.message)?;
        }

        Ok(())
    }

    async fn persist_session(&self, session_id: &SessionId) -> Result<(), String> {
        let persisted = {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;

            rebuild_materialized_from_event_log(session);

            PersistedSession {
                id: session.id.clone(),
                config: session.config.clone(),
                captain: session.captain.clone(),
                mate: session.mate.clone(),
                current_task: session.current_task.clone(),
                task_history: session.task_history.clone(),
            }
        };

        self.store
            .save_session(&persisted)
            .await
            .map_err(|error| format!("store error: {}", error.message))
    }

    async fn drain_notifications(&self, session_id: &SessionId, role: Role) -> Result<(), String> {
        let handle = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            match role {
                Role::Captain => session.captain_handle.clone(),
                Role::Mate => session.mate_handle.clone(),
            }
        };

        let mut stream = self.agent_driver.notifications(&handle);
        while let Some(event) = stream.next().await {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(session) = sessions.get_mut(session_id) else {
                break;
            };
            apply_event(session, event);
        }
        drop(stream);

        Ok(())
    }

    async fn prompt_agent(
        &self,
        session_id: &SessionId,
        role: Role,
        prompt: String,
    ) -> Result<ship_core::StopReason, String> {
        let handle = {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
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

        self.persist_session(session_id).await?;

        let response = self
            .agent_driver
            .prompt(&handle, &prompt)
            .await
            .map_err(|error| error.message)?;

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            match response.stop_reason {
                ship_core::StopReason::ContextExhausted => {
                    set_agent_state(session, role, AgentState::ContextExhausted)
                }
                _ => set_agent_state(session, role, AgentState::Idle),
            }
        }

        self.drain_notifications(session_id, role).await?;
        self.persist_session(session_id).await?;

        Ok(response.stop_reason)
    }

    async fn prompt_captain_review(&self, session_id: &SessionId) -> Result<(), String> {
        let prompt = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let task = session
                .current_task
                .as_ref()
                .ok_or_else(|| "session has no active task".to_owned())?;
            format!(
                "Review mate output for task:\n{}\n\nProvide steer or acceptance.",
                task.record.description
            )
        };

        self.prompt_agent(session_id, Role::Captain, prompt)
            .await
            .map(|_| ())
    }

    async fn handle_mate_stop_reason(
        &self,
        session_id: &SessionId,
        stop_reason: ship_core::StopReason,
    ) -> Result<(), String> {
        match stop_reason {
            ship_core::StopReason::EndTurn => {
                {
                    let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    let session = sessions
                        .get_mut(session_id)
                        .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                    transition_task(session, TaskStatus::ReviewPending)
                        .map_err(|error| error.to_string())?;
                }
                self.persist_session(session_id).await?;
                self.prompt_captain_review(session_id).await?;
            }
            ship_core::StopReason::Cancelled => {
                {
                    let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    let session = sessions
                        .get_mut(session_id)
                        .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                    transition_task(session, TaskStatus::Cancelled)
                        .map_err(|error| error.to_string())?;
                    archive_terminal_task(session);
                }
                self.persist_session(session_id).await?;
            }
            ship_core::StopReason::ContextExhausted => {
                {
                    let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    let session = sessions
                        .get_mut(session_id)
                        .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                    set_agent_state(session, Role::Mate, AgentState::ContextExhausted);
                }
                self.persist_session(session_id).await?;
            }
        }

        Ok(())
    }

    async fn run_task_prompt_flow(&self, session_id: SessionId, task_description: String) {
        let captain_prompt =
            format!("Provide implementation direction for this task:\n{task_description}");

        if let Err(error) = self
            .prompt_agent(&session_id, Role::Captain, captain_prompt)
            .await
        {
            Self::log_error("prompt_captain_initial", &error);
            return;
        }

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(session) = sessions.get_mut(&session_id) else {
                return;
            };
            if let Err(error) = transition_task(session, TaskStatus::Working) {
                Self::log_error("transition_after_captain", &error.to_string());
                return;
            }
        }

        if let Err(error) = self.persist_session(&session_id).await {
            Self::log_error("persist_after_captain", &error);
            return;
        }

        let mate_prompt = format!(
            "Task:\n{task_description}\n\nCaptain direction:\nProvide an implementation plan and execute."
        );

        let stop_reason = match self
            .prompt_agent(&session_id, Role::Mate, mate_prompt)
            .await
        {
            Ok(stop_reason) => stop_reason,
            Err(error) => {
                Self::log_error("prompt_mate", &error);
                return;
            }
        };

        if let Err(error) = self.handle_mate_stop_reason(&session_id, stop_reason).await {
            Self::log_error("handle_mate_stop_reason", &error);
        }
    }

    fn spawn_task_flow(&self, session_id: SessionId, task_description: String) {
        let this = self.clone();
        tokio::spawn(async move {
            this.run_task_prompt_flow(session_id, task_description)
                .await;
        });
    }

    async fn start_task(
        &self,
        session_id: &SessionId,
        description: String,
    ) -> Result<TaskId, String> {
        let task_id = TaskId::new();

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            if session.current_task.is_some() {
                return Err("session already has an active non-terminal task".to_owned());
            }

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
        self.spawn_task_flow(session_id.clone(), description);

        Ok(task_id)
    }

    async fn prompt_mate_from_steer(&self, session_id: SessionId, message: String) {
        let stop_reason = match self
            .prompt_agent(
                &session_id,
                Role::Mate,
                format!("Captain steer:\n{message}"),
            )
            .await
        {
            Ok(stop_reason) => stop_reason,
            Err(error) => {
                Self::log_error("prompt_mate_steer", &error);
                return;
            }
        };

        if let Err(error) = self.handle_mate_stop_reason(&session_id, stop_reason).await {
            Self::log_error("handle_mate_stop_reason_steer", &error);
        }
    }
}

impl Ship for ShipImpl {
    async fn list_projects(&self) -> Vec<ProjectInfo> {
        self.registry.lock().await.list()
    }

    async fn add_project(&self, path: String) -> ProjectInfo {
        let mut registry = self.registry.lock().await;
        match registry.add(&path).await {
            Ok(project) => project,
            Err(error) => ProjectInfo {
                name: ProjectName(
                    path.rsplit('/')
                        .find(|segment| !segment.is_empty())
                        .unwrap_or("project")
                        .to_owned(),
                ),
                path,
                valid: false,
                invalid_reason: Some(error.to_string()),
            },
        }
    }

    async fn list_branches(&self, project: ProjectName) -> Vec<String> {
        let project_path = {
            let registry = self.registry.lock().await;
            registry.get(&project.0).map(|project| project.path)
        };

        let Some(project_path) = project_path else {
            return Vec::new();
        };

        let output = Command::new("git")
            .args(["-C", project_path.as_str(), "branch", "-a"])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .map(|line| line.strip_prefix("* ").unwrap_or(line))
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    async fn list_sessions(&self) -> Vec<SessionSummary> {
        let sessions = self.sessions.lock().expect("sessions mutex poisoned");
        sessions.values().map(Self::to_session_summary).collect()
    }

    async fn get_session(&self, id: SessionId) -> SessionDetail {
        let sessions = self.sessions.lock().expect("sessions mutex poisoned");
        sessions
            .get(&id)
            .map(Self::to_session_detail)
            .unwrap_or_else(|| Self::fallback_session_detail(id))
    }

    async fn create_session(&self, req: CreateSessionRequest) -> CreateSessionResponse {
        let session_id = SessionId::new();
        let slug = slugify(&req.task_description);
        let worktree_path = match self
            .worktree_ops
            .create_worktree(
                &session_id,
                &req.base_branch,
                &slug,
                self.repo_root.as_ref(),
            )
            .await
        {
            Ok(path) => path,
            Err(error) => {
                Self::log_error("create_worktree", &error.message);
                return CreateSessionResponse {
                    session_id: SessionId::new(),
                    task_id: TaskId::new(),
                };
            }
        };

        let branch_name = format!("ship/{}/{slug}", short_id(&session_id));

        let captain_handle = match self
            .agent_driver
            .spawn(req.captain_kind, Role::Captain, &worktree_path)
            .await
        {
            Ok(handle) => handle,
            Err(error) => {
                Self::log_error("spawn_captain", &error.message);
                return CreateSessionResponse {
                    session_id: SessionId::new(),
                    task_id: TaskId::new(),
                };
            }
        };

        let mate_handle = match self
            .agent_driver
            .spawn(req.mate_kind, Role::Mate, &worktree_path)
            .await
        {
            Ok(handle) => handle,
            Err(error) => {
                Self::log_error("spawn_mate", &error.message);
                return CreateSessionResponse {
                    session_id: SessionId::new(),
                    task_id: TaskId::new(),
                };
            }
        };

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

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            sessions.insert(session_id.clone(), session);
        }

        let task_id = match self.start_task(&session_id, req.task_description).await {
            Ok(task_id) => task_id,
            Err(error) => {
                Self::log_error("start_task", &error);
                TaskId::new()
            }
        };

        CreateSessionResponse {
            session_id,
            task_id,
        }
    }

    async fn assign(&self, session: SessionId, description: String) -> TaskId {
        match self.start_task(&session, description).await {
            Ok(task_id) => task_id,
            Err(error) => {
                Self::log_error("assign", &error);
                TaskId::new()
            }
        }
    }

    async fn steer(&self, session: SessionId, content: String) {
        let mode = {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(active) = sessions.get_mut(&session) else {
                Self::log_error("steer", "session not found");
                return;
            };

            let status = match current_task_status(active) {
                Ok(status) => status,
                Err(error) => {
                    Self::log_error("steer", &error.to_string());
                    return;
                }
            };
            if status != TaskStatus::ReviewPending && status != TaskStatus::SteerPending {
                Self::log_error("steer", "invalid task transition");
                return;
            }

            apply_event(
                active,
                SessionEvent::BlockAppend {
                    block_id: BlockId::new(),
                    role: Role::Captain,
                    block: ContentBlock::Text {
                        text: content.clone(),
                    },
                },
            );

            if active.config.autonomy_mode == AutonomyMode::Autonomous {
                if let Err(error) = transition_task(active, TaskStatus::Working) {
                    Self::log_error("steer", &error.to_string());
                    return;
                }
            } else {
                if let Err(error) = transition_task(active, TaskStatus::SteerPending) {
                    Self::log_error("steer", &error.to_string());
                    return;
                }
                active.pending_steer = Some(content.clone());
            }

            active.config.autonomy_mode
        };

        if let Err(error) = self.persist_session(&session).await {
            Self::log_error("steer_persist", &error);
            return;
        }

        if mode == AutonomyMode::Autonomous {
            let this = self.clone();
            tokio::spawn(async move {
                this.prompt_mate_from_steer(session, content).await;
            });
        }
    }

    async fn accept(&self, session: SessionId) {
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(active) = sessions.get_mut(&session) else {
                Self::log_error("accept", "session not found");
                return;
            };
            let status = match current_task_status(active) {
                Ok(status) => status,
                Err(error) => {
                    Self::log_error("accept", &error.to_string());
                    return;
                }
            };
            if status != TaskStatus::ReviewPending {
                Self::log_error("accept", "invalid task transition");
                return;
            }

            if let Err(error) = transition_task(active, TaskStatus::Accepted) {
                Self::log_error("accept", &error.to_string());
                return;
            }
            archive_terminal_task(active);
        }

        if let Err(error) = self.persist_session(&session).await {
            Self::log_error("accept_persist", &error);
        }
    }

    async fn cancel(&self, session: SessionId) {
        let mate_handle = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(active) = sessions.get(&session) else {
                Self::log_error("cancel", "session not found");
                return;
            };
            if active
                .current_task
                .as_ref()
                .map(|task| task.record.status.is_terminal())
                .unwrap_or(true)
            {
                Self::log_error("cancel", "session has no active task");
                return;
            }
            active.mate_handle.clone()
        };

        if let Err(error) = self.agent_driver.cancel(&mate_handle).await {
            Self::log_error("cancel", &error.message);
            return;
        }

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(active) = sessions.get_mut(&session) else {
                Self::log_error("cancel", "session not found after cancel");
                return;
            };
            if let Err(error) = transition_task(active, TaskStatus::Cancelled) {
                Self::log_error("cancel", &error.to_string());
                return;
            }
            archive_terminal_task(active);
        }

        if let Err(error) = self.persist_session(&session).await {
            Self::log_error("cancel_persist", &error);
        }
    }

    async fn resolve_permission(&self, session: SessionId, permission_id: String, approved: bool) {
        let (pending_role, handle) = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(active) = sessions.get(&session) else {
                Self::log_error("resolve_permission", "session not found");
                return;
            };

            let Some(pending) = active.pending_permissions.get(&permission_id) else {
                Self::log_error("resolve_permission", "permission not found");
                return;
            };

            let handle = match pending.role {
                Role::Captain => active.captain_handle.clone(),
                Role::Mate => active.mate_handle.clone(),
            };
            (pending.role, handle)
        };

        if let Err(error) = self
            .agent_driver
            .resolve_permission(&handle, &permission_id, approved)
            .await
        {
            Self::log_error("resolve_permission", &error.message);
            return;
        }

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(active) = sessions.get_mut(&session) else {
                Self::log_error("resolve_permission", "session missing after driver call");
                return;
            };

            set_agent_state(
                active,
                pending_role,
                AgentState::Working {
                    plan: None,
                    activity: Some("Permission resolved".to_owned()),
                },
            );
            apply_event(
                active,
                SessionEvent::BlockPatch {
                    block_id: BlockId(permission_id.clone()),
                    role: pending_role,
                    patch: ship_types::BlockPatch::PermissionResolve {
                        resolution: if approved {
                            ship_types::PermissionResolution::Approved
                        } else {
                            ship_types::PermissionResolution::Denied
                        },
                    },
                },
            );
        }

        if let Err(error) = self.persist_session(&session).await {
            Self::log_error("resolve_permission_persist", &error);
        }
    }

    async fn retry_agent(&self, _session: SessionId, _role: Role) {}

    // r[backend.worktree-management]
    // r[worktree.cleanup]
    // r[worktree.cleanup-uncommitted]
    // r[worktree.cleanup-git]
    async fn close_session(&self, req: CloseSessionRequest) -> CloseSessionResponse {
        let session = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(session) = sessions.get(&req.id) else {
                Self::log_error("close_session", "session not found");
                return CloseSessionResponse::NotFound;
            };
            session.clone()
        };
        let worktree_path = session.worktree_path.clone();

        match async {
            if worktree_path.exists() {
                self.worktree_ops
                    .has_uncommitted_changes(&worktree_path)
                    .await
            } else {
                Ok(false)
            }
        }
        .await
        {
            Ok(true) if !req.force => return CloseSessionResponse::RequiresConfirmation,
            Ok(_) => {}
            Err(error) => {
                Self::log_error("close_session_has_uncommitted_changes", &error.message);
                return CloseSessionResponse::Failed {
                    message: error.message,
                };
            }
        }

        if let Err(error) = self.cleanup_session_resources(&session, req.force).await {
            Self::log_error("close_session_cleanup", &error);
            return CloseSessionResponse::Failed { message: error };
        }
        if let Err(error) = self.store.delete_session(&req.id).await {
            Self::log_error("close_session_delete_session", &error.message);
            return CloseSessionResponse::Failed {
                message: error.message,
            };
        }
        self.sessions
            .lock()
            .expect("sessions mutex poisoned")
            .remove(&req.id);

        CloseSessionResponse::Closed
    }

    async fn subscribe_events(&self, session: SessionId, output: Tx<SubscribeMessage>) {
        let session_data = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            sessions.get(&session).map(|active| {
                let replay = active
                    .current_task
                    .as_ref()
                    .map(|task| task.event_log.clone())
                    .unwrap_or_default();
                (active.events_tx.subscribe(), replay)
            })
        };
        let Some((mut receiver, replay)) = session_data else {
            let _ = output.close(Default::default()).await;
            return;
        };

        for event in replay {
            if output.send(SubscribeMessage::Event(event)).await.is_err() {
                return;
            }
        }

        if output.send(SubscribeMessage::ReplayComplete).await.is_err() {
            return;
        }

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if output.send(SubscribeMessage::Event(event)).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(%skipped, "subscribe live stream lagged");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    let _ = output.close(Default::default()).await;
                    return;
                }
            }
        }
    }
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

    slug.trim_matches('-').to_owned()
}
