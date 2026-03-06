use std::collections::HashMap;
use std::future::Future;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use roam::Tx;
use ship_core::{
    AcpAgentDriver, ActiveSession, AgentDriver, AgentSessionConfig, GitWorktreeOps,
    JsonSessionStore, ProjectRegistry, SessionStore, WorktreeOps, apply_event,
    archive_terminal_task, current_task_status, rebuild_materialized_from_event_log,
    resolve_mcp_servers, set_agent_state, transition_task,
};
use ship_service::Ship;
use ship_types::{
    AgentDiscovery, AgentKind, AgentSnapshot, AgentState, AutonomyMode, BlockId,
    CloseSessionRequest, CloseSessionResponse, ContentBlock, CreateSessionRequest,
    CreateSessionResponse, CurrentTask, McpServerConfig, PersistedSession, ProjectInfo,
    ProjectName, Role, SessionConfig, SessionDetail, SessionEvent, SessionId, SessionSummary,
    SubscribeMessage, TaskId, TaskRecord, TaskStatus,
};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

// r[server.multi-repo]
#[derive(Clone)]
pub struct ShipImpl {
    registry: Arc<tokio::sync::Mutex<ProjectRegistry>>,
    agent_discovery: AgentDiscovery,
    agent_driver: Arc<AcpAgentDriver>,
    worktree_ops: Arc<GitWorktreeOps>,
    store: Arc<JsonSessionStore>,
    sessions: Arc<Mutex<HashMap<SessionId, ActiveSession>>>,
}

impl ShipImpl {
    pub fn new(
        registry: ProjectRegistry,
        sessions_dir: std::path::PathBuf,
        agent_discovery: AgentDiscovery,
    ) -> Self {
        Self {
            registry: Arc::new(tokio::sync::Mutex::new(registry)),
            agent_discovery,
            agent_driver: Arc::new(AcpAgentDriver::new()),
            worktree_ops: Arc::new(GitWorktreeOps),
            store: Arc::new(JsonSessionStore::new(sessions_dir)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
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

    fn event_kind(event: &SessionEvent) -> &'static str {
        match event {
            SessionEvent::BlockAppend { .. } => "BlockAppend",
            SessionEvent::BlockPatch { .. } => "BlockPatch",
            SessionEvent::AgentStateChanged { .. } => "AgentStateChanged",
            SessionEvent::TaskStatusChanged { .. } => "TaskStatusChanged",
            SessionEvent::ContextUpdated { .. } => "ContextUpdated",
            SessionEvent::TaskStarted { .. } => "TaskStarted",
        }
    }

    fn event_role(event: &SessionEvent) -> Option<Role> {
        match event {
            SessionEvent::BlockAppend { role, .. }
            | SessionEvent::BlockPatch { role, .. }
            | SessionEvent::AgentStateChanged { role, .. }
            | SessionEvent::ContextUpdated { role, .. } => Some(*role),
            SessionEvent::TaskStatusChanged { .. } | SessionEvent::TaskStarted { .. } => None,
        }
    }

    fn event_block_id(event: &SessionEvent) -> Option<&str> {
        match event {
            SessionEvent::BlockAppend { block_id, .. }
            | SessionEvent::BlockPatch { block_id, .. } => Some(&block_id.0),
            SessionEvent::AgentStateChanged { .. }
            | SessionEvent::TaskStatusChanged { .. }
            | SessionEvent::ContextUpdated { .. }
            | SessionEvent::TaskStarted { .. } => None,
        }
    }

    fn event_task_id(event: &SessionEvent) -> Option<&str> {
        match event {
            SessionEvent::TaskStatusChanged { task_id, .. }
            | SessionEvent::TaskStarted { task_id, .. } => Some(&task_id.0),
            SessionEvent::BlockAppend { .. }
            | SessionEvent::BlockPatch { .. }
            | SessionEvent::AgentStateChanged { .. }
            | SessionEvent::ContextUpdated { .. } => None,
        }
    }

    fn spawn_event_subscription(
        session: SessionId,
        receiver: broadcast::Receiver<ship_types::SessionEventEnvelope>,
        replay: Vec<ship_types::SessionEventEnvelope>,
        output: Tx<SubscribeMessage>,
    ) {
        tokio::spawn(async move {
            Self::forward_event_subscription(
                &session,
                receiver,
                replay,
                |message| {
                    let output = &output;
                    async move { output.send(message).await.map_err(|_| ()) }
                },
                || {
                    let output = &output;
                    async move {
                        let _ = output.close(Default::default()).await;
                    }
                },
            )
            .await;
        });
    }

    async fn forward_event_subscription<Send, SendFuture, Close, CloseFuture>(
        session: &SessionId,
        mut receiver: broadcast::Receiver<ship_types::SessionEventEnvelope>,
        replay: Vec<ship_types::SessionEventEnvelope>,
        mut send: Send,
        mut close: Close,
    ) where
        Send: FnMut(SubscribeMessage) -> SendFuture,
        SendFuture: Future<Output = Result<(), ()>>,
        Close: FnMut() -> CloseFuture,
        CloseFuture: Future<Output = ()>,
    {
        tracing::info!(
            session_id = %session.0,
            replay_events = replay.len(),
            "starting event replay"
        );

        for event in replay {
            tracing::debug!(
                session_id = %session.0,
                seq = event.seq,
                event_kind = Self::event_kind(&event.event),
                role = ?Self::event_role(&event.event),
                block_id = Self::event_block_id(&event.event),
                task_id = Self::event_task_id(&event.event),
                "sending replay event to subscriber"
            );
            if send(SubscribeMessage::Event(event)).await.is_err() {
                tracing::warn!(session_id = %session.0, "subscriber disconnected during replay");
                return;
            }
        }

        if send(SubscribeMessage::ReplayComplete).await.is_err() {
            tracing::warn!(
                session_id = %session.0,
                "subscriber disconnected before replay completion marker"
            );
            return;
        }
        tracing::info!(session_id = %session.0, "replay complete");

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    tracing::debug!(
                        session_id = %session.0,
                        seq = event.seq,
                        event_kind = Self::event_kind(&event.event),
                        role = ?Self::event_role(&event.event),
                        block_id = Self::event_block_id(&event.event),
                        task_id = Self::event_task_id(&event.event),
                        "sending live event to subscriber"
                    );
                    if send(SubscribeMessage::Event(event)).await.is_err() {
                        tracing::warn!(
                            session_id = %session.0,
                            "subscriber disconnected during live stream"
                        );
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(session_id = %session.0, %skipped, "subscribe live stream lagged");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!(session_id = %session.0, "session event stream closed");
                    close().await;
                    return;
                }
            }
        }
    }

    async fn cleanup_partial_creation(
        &self,
        repo_root: &std::path::Path,
        worktree_path: &std::path::Path,
        branch_name: &str,
        captain_handle: Option<&ship_core::AgentHandle>,
        mate_handle: Option<&ship_core::AgentHandle>,
    ) {
        if let Some(handle) = captain_handle
            && let Err(error) = self.agent_driver.kill(handle).await
        {
            tracing::warn!(%branch_name, error = %error.message, "failed to kill captain during session creation rollback");
        }
        if let Some(handle) = mate_handle
            && let Err(error) = self.agent_driver.kill(handle).await
        {
            tracing::warn!(%branch_name, error = %error.message, "failed to kill mate during session creation rollback");
        }

        if worktree_path.exists()
            && let Err(error) = self.worktree_ops.remove_worktree(worktree_path, true).await
        {
            tracing::warn!(
                worktree_path = %worktree_path.display(),
                error = %error.message,
                "failed to remove worktree during session creation rollback"
            );
        }

        match self.worktree_ops.list_branches(repo_root).await {
            Ok(branches) if branches.iter().any(|branch| branch == branch_name) => {
                if let Err(error) = self
                    .worktree_ops
                    .delete_branch(branch_name, true, repo_root)
                    .await
                {
                    tracing::warn!(%branch_name, error = %error.message, "failed to delete branch during session creation rollback");
                }
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%branch_name, error = %error.message, "failed to inspect branches during session creation rollback");
            }
        }
    }

    async fn rollback_created_session(&self, session_id: &SessionId) {
        let session = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            sessions.get(session_id).cloned()
        };

        if let Some(session) = session
            && let Err(error) = self.cleanup_session_resources(&session, true).await
        {
            tracing::warn!(session_id = %session_id.0, %error, "failed to clean up rolled back session resources");
        }

        self.sessions
            .lock()
            .expect("sessions mutex poisoned")
            .remove(session_id);

        if let Err(error) = self.store.delete_session(session_id).await {
            tracing::warn!(session_id = %session_id.0, error = %error.message, "failed to delete rolled back persisted session");
        }
    }

    // r[acp.mcp.config]
    // r[acp.mcp.defaults]
    // r[project.mcp-defaults]
    async fn resolve_session_mcp_servers(
        &self,
        project: &ProjectName,
        session_override: Option<Vec<McpServerConfig>>,
    ) -> Result<(std::path::PathBuf, Vec<McpServerConfig>), String> {
        let (config_dir, project_root) = {
            let registry = self.registry.lock().await;
            let config_dir = registry.config_dir().to_path_buf();
            let project = registry
                .get(&project.0)
                .ok_or_else(|| format!("project not found: {}", project.0))?;
            (config_dir, std::path::PathBuf::from(project.path))
        };

        let mcp_servers = resolve_mcp_servers(&config_dir, &project_root, session_override)
            .await
            .map_err(|error| error.message)?;

        Ok((project_root, mcp_servers))
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
            let event_seq = {
                let sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let Some(session) = sessions.get(session_id) else {
                    break;
                };
                session.next_event_seq
            };
            tracing::debug!(
                session_id = %session_id.0,
                seq = event_seq,
                role = ?role,
                event_kind = Self::event_kind(&event),
                block_id = Self::event_block_id(&event),
                task_id = Self::event_task_id(&event),
                "applying agent notification"
            );
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(session) = sessions.get_mut(session_id) else {
                break;
            };
            apply_event(session, event);
        }
        drop(stream);

        Ok(())
    }

    fn spawn_notification_pump(
        &self,
        session_id: SessionId,
        role: Role,
    ) -> (tokio::sync::oneshot::Sender<()>, JoinHandle<()>) {
        let this = self.clone();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            tracing::debug!(session_id = %session_id.0, role = ?role, "starting prompt notification pump");
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {
                        if let Err(error) = this.drain_notifications(&session_id, role).await {
                            tracing::warn!(session_id = %session_id.0, role = ?role, %error, "notification pump failed");
                            break;
                        }
                    }
                }
            }

            if let Err(error) = this.drain_notifications(&session_id, role).await {
                tracing::warn!(session_id = %session_id.0, role = ?role, %error, "final notification drain failed");
            }
            tracing::debug!(session_id = %session_id.0, role = ?role, "stopped prompt notification pump");
        });
        (stop_tx, handle)
    }

    async fn prompt_agent(
        &self,
        session_id: &SessionId,
        role: Role,
        prompt: String,
    ) -> Result<ship_core::StopReason, String> {
        tracing::info!(session_id = %session_id.0, role = ?role, prompt_len = prompt.len(), "starting agent prompt");
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

        let (stop_tx, pump_handle) = self.spawn_notification_pump(session_id.clone(), role);
        let response = self
            .agent_driver
            .prompt(&handle, &prompt)
            .await
            .map_err(|error| error.message)?;
        let _ = stop_tx.send(());
        let _ = pump_handle.await;

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

        self.persist_session(session_id).await?;
        tracing::info!(session_id = %session_id.0, role = ?role, stop_reason = ?response.stop_reason, "agent prompt completed");

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
        tracing::info!(session_id = %session_id.0, task_description, "starting task prompt flow");
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
        tracing::info!(session_id = %session_id.0, "task prompt flow finished");
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
        tracing::info!(session_id = %session_id.0, task_id = %task_id.0, "starting task");

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

    async fn agent_discovery(&self) -> AgentDiscovery {
        self.agent_discovery.clone()
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
        let branch_name = format!("ship/{}/{slug}", short_id(&session_id));
        tracing::info!(
            session_id = %session_id.0,
            project = %req.project.0,
            base_branch = %req.base_branch,
            captain_kind = ?req.captain_kind,
            mate_kind = ?req.mate_kind,
            "create_session requested"
        );
        let (repo_root, mcp_servers) = match self
            .resolve_session_mcp_servers(&req.project, req.mcp_servers.clone())
            .await
        {
            Ok(resolved) => resolved,
            Err(error) => {
                Self::log_error("resolve_session_mcp_servers", &error);
                return CreateSessionResponse::Failed { message: error };
            }
        };
        tracing::info!(session_id = %session_id.0, repo_root = %repo_root.display(), "resolved project and MCP configuration");
        let worktree_path = match self
            .worktree_ops
            .create_worktree(&session_id, &req.base_branch, &slug, &repo_root)
            .await
        {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(
                    session_id = %session_id.0,
                    base_branch = %req.base_branch,
                    error = %error.message,
                    "failed to create worktree"
                );
                return CreateSessionResponse::Failed {
                    message: error.message,
                };
            }
        };
        tracing::info!(
            session_id = %session_id.0,
            branch_name = %branch_name,
            worktree_path = %worktree_path.display(),
            "created session worktree"
        );
        let agent_session_config = AgentSessionConfig {
            worktree_path: worktree_path.clone(),
            mcp_servers: mcp_servers.clone(),
        };

        tracing::info!(session_id = %session_id.0, role = ?Role::Captain, agent_kind = ?req.captain_kind, "spawning agent");
        let captain_handle = match self
            .agent_driver
            .spawn(req.captain_kind, Role::Captain, &agent_session_config)
            .await
        {
            Ok(handle) => handle,
            Err(error) => {
                tracing::warn!(session_id = %session_id.0, role = ?Role::Captain, error = %error.message, "failed to spawn agent");
                self.cleanup_partial_creation(&repo_root, &worktree_path, &branch_name, None, None)
                    .await;
                return CreateSessionResponse::Failed {
                    message: format!("failed to spawn captain agent: {}", error.message),
                };
            }
        };
        tracing::info!(session_id = %session_id.0, role = ?Role::Captain, "agent spawned");

        tracing::info!(session_id = %session_id.0, role = ?Role::Mate, agent_kind = ?req.mate_kind, "spawning agent");
        let mate_handle = match self
            .agent_driver
            .spawn(req.mate_kind, Role::Mate, &agent_session_config)
            .await
        {
            Ok(handle) => handle,
            Err(error) => {
                tracing::warn!(session_id = %session_id.0, role = ?Role::Mate, error = %error.message, "failed to spawn agent");
                self.cleanup_partial_creation(
                    &repo_root,
                    &worktree_path,
                    &branch_name,
                    Some(&captain_handle),
                    None,
                )
                .await;
                return CreateSessionResponse::Failed {
                    message: format!("failed to spawn mate agent: {}", error.message),
                };
            }
        };
        tracing::info!(session_id = %session_id.0, role = ?Role::Mate, "agent spawned");

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
                mcp_servers,
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
        tracing::info!(session_id = %session_id.0, "session inserted into active map");

        let task_id = match self.start_task(&session_id, req.task_description).await {
            Ok(task_id) => task_id,
            Err(error) => {
                tracing::warn!(session_id = %session_id.0, %error, "session creation failed while starting first task");
                self.rollback_created_session(&session_id).await;
                return CreateSessionResponse::Failed {
                    message: format!("failed to start initial task: {error}"),
                };
            }
        };
        tracing::info!(session_id = %session_id.0, task_id = %task_id.0, "create_session completed");

        CreateSessionResponse::Created {
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

    // r[acp.prompt]
    async fn prompt_captain(&self, session: SessionId, content: String) {
        if let Err(error) = self.prompt_agent(&session, Role::Captain, content).await {
            Self::log_error("prompt_captain", &error);
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
        tracing::info!(session_id = %session.0, "subscriber connected");
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
        let Some((receiver, replay)) = session_data else {
            tracing::warn!(session_id = %session.0, "subscribe requested for unknown session");
            let _ = output.close(Default::default()).await;
            return;
        };
        Self::spawn_event_subscription(session, receiver, replay, output);
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use ship_core::ProjectRegistry;
    use ship_service::Ship;
    use ship_types::{
        AgentDiscovery, AgentKind, CreateSessionRequest, CreateSessionResponse, McpServerConfig,
        McpStdioServerConfig, ProjectName, SessionEvent, SessionEventEnvelope, SessionId,
        SubscribeMessage, TaskId,
    };
    use tokio::sync::{broadcast, mpsc};
    use tokio::time::timeout;

    use super::ShipImpl;

    fn make_temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ship-impl-{test_name}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    // r[verify server.agent-discovery]
    #[tokio::test]
    async fn service_returns_startup_agent_discovery_snapshot() {
        let dir = make_temp_dir("agent-discovery");
        let registry = ProjectRegistry::load_in(dir.join("config"))
            .await
            .expect("project registry should load");
        let expected = AgentDiscovery {
            claude: true,
            codex: false,
        };
        let ship = ShipImpl::new(registry, dir.join("sessions"), expected.clone());

        assert_eq!(Ship::agent_discovery(&ship).await, expected);

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify acp.mcp.defaults]
    // r[verify project.mcp-defaults]
    #[tokio::test]
    async fn resolve_session_mcp_servers_prefers_project_defaults() {
        let dir = make_temp_dir("mcp-defaults");
        let config_dir = dir.join("config");
        let project_root = dir.join("project");
        std::fs::create_dir_all(&config_dir).expect("config dir should exist");
        std::fs::create_dir_all(project_root.join(".ship")).expect("project ship dir should exist");
        std::fs::write(
            config_dir.join("mcp-servers.json"),
            r#"[{"name":"global","command":"/usr/bin/global-mcp","args":[],"env":[]}]"#,
        )
        .expect("global mcp defaults should be written");
        std::fs::write(
            project_root.join(".ship/mcp-servers.json"),
            r#"[{"name":"project","command":"/usr/bin/project-mcp","args":[],"env":[]}]"#,
        )
        .expect("project mcp defaults should be written");

        let mut registry = ProjectRegistry::load_in(config_dir.clone())
            .await
            .expect("project registry should load");
        registry
            .add(&project_root)
            .await
            .expect("project should be added");

        let ship = ShipImpl::new(
            registry,
            dir.join("sessions"),
            AgentDiscovery {
                claude: true,
                codex: true,
            },
        );

        let (resolved_root, mcp_servers) = ship
            .resolve_session_mcp_servers(&ProjectName("project".to_owned()), None)
            .await
            .expect("mcp defaults should resolve");

        assert_eq!(resolved_root, project_root);
        assert_eq!(
            mcp_servers,
            vec![McpServerConfig::Stdio(McpStdioServerConfig {
                name: "project".to_owned(),
                command: "/usr/bin/project-mcp".to_owned(),
                args: Vec::new(),
                env: Vec::new(),
            })]
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify acp.mcp.config]
    #[tokio::test]
    async fn create_session_returns_failure_when_mcp_config_is_invalid() {
        let dir = make_temp_dir("create-session-invalid-mcp");
        let config_dir = dir.join("config");
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join(".ship")).expect("project ship dir should exist");
        std::fs::write(project_root.join(".ship/mcp-servers.json"), "{invalid")
            .expect("invalid project mcp defaults should be written");

        let mut registry = ProjectRegistry::load_in(config_dir)
            .await
            .expect("project registry should load");
        registry
            .add(&project_root)
            .await
            .expect("project should be added");

        let ship = ShipImpl::new(
            registry,
            dir.join("sessions"),
            AgentDiscovery {
                claude: true,
                codex: true,
            },
        );

        let response = Ship::create_session(
            &ship,
            CreateSessionRequest {
                project: ProjectName("project".to_owned()),
                captain_kind: AgentKind::Claude,
                mate_kind: AgentKind::Codex,
                base_branch: "main".to_owned(),
                task_description: "broken".to_owned(),
                mcp_servers: None,
            },
        )
        .await;

        assert!(matches!(response, CreateSessionResponse::Failed { .. }));
        assert!(Ship::list_sessions(&ship).await.is_empty());

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify event.subscribe.roam-channel]
    // r[verify event.subscribe.replay]
    // r[verify event.replay.followed-by-marker]
    #[tokio::test]
    async fn spawned_subscription_keeps_streaming_after_setup_returns() {
        let session_id = SessionId::new();
        let task_id = TaskId::new();
        let live_task_id = TaskId::new();
        let (live_tx, live_rx) = broadcast::channel(16);
        let (messages_tx, mut messages_rx) = mpsc::unbounded_channel();
        let replay = vec![SessionEventEnvelope {
            seq: 7,
            event: SessionEvent::TaskStarted {
                task_id: task_id.clone(),
                description: "Replay task".to_owned(),
            },
        }];

        tokio::spawn(async move {
            ShipImpl::forward_event_subscription(
                &session_id,
                live_rx,
                replay,
                |message| {
                    let messages_tx = messages_tx.clone();
                    async move { messages_tx.send(message).map_err(|_| ()) }
                },
                || async {},
            )
            .await;
        });

        let replayed = timeout(Duration::from_secs(1), messages_rx.recv())
            .await
            .expect("replay should arrive")
            .expect("replay event should be present");
        assert_eq!(
            replayed,
            SubscribeMessage::Event(SessionEventEnvelope {
                seq: 7,
                event: SessionEvent::TaskStarted {
                    task_id: task_id.clone(),
                    description: "Replay task".to_owned(),
                },
            })
        );

        let replay_complete = timeout(Duration::from_secs(1), messages_rx.recv())
            .await
            .expect("replay marker should arrive")
            .expect("replay marker should be present");
        assert_eq!(replay_complete, SubscribeMessage::ReplayComplete);

        live_tx
            .send(SessionEventEnvelope {
                seq: 8,
                event: SessionEvent::TaskStarted {
                    task_id: live_task_id.clone(),
                    description: "Live task".to_owned(),
                },
            })
            .expect("live send should succeed");

        let live = timeout(Duration::from_secs(1), messages_rx.recv())
            .await
            .expect("live event should arrive")
            .expect("live event should be present");
        assert_eq!(
            live,
            SubscribeMessage::Event(SessionEventEnvelope {
                seq: 8,
                event: SessionEvent::TaskStarted {
                    task_id: live_task_id,
                    description: "Live task".to_owned(),
                },
            })
        );
    }
}
