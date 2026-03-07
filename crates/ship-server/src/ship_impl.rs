use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use roam::Tx;
use roam::{
    AcceptedConnection, ConnectionAcceptor, ConnectionSettings, Driver, Metadata, MetadataEntry,
    MetadataFlags, MetadataValue,
};
use ship_core::{
    AcpAgentDriver, ActiveSession, AgentDriver, AgentSessionConfig, GitWorktreeOps,
    JsonSessionStore, ProjectRegistry, SessionStore, WorktreeOps, apply_event,
    archive_terminal_task, current_task_status, rebuild_materialized_from_event_log,
    resolve_mcp_servers, set_agent_state, transition_task,
};
use ship_service::{CaptainMcp, CaptainMcpDispatcher, MateMcp, MateMcpDispatcher, Ship};
use ship_types::{
    AgentDiscovery, AgentKind, AgentSnapshot, AgentState, AutonomyMode, BlockId,
    CloseSessionRequest, CloseSessionResponse, ContentBlock, CreateSessionRequest,
    CreateSessionResponse, CurrentTask, McpServerConfig, McpStdioServerConfig, McpToolCallResponse,
    PersistedSession, ProjectInfo, ProjectName, Role, SessionConfig, SessionDetail, SessionEvent,
    SessionId, SessionStartupStage, SessionStartupState, SessionSummary, SetAgentModelResponse,
    SubscribeMessage, TaskId, TaskRecord, TaskStatus,
};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

pub enum MateReviewOutcome {
    Accepted { summary: Option<String> },
    Feedback { message: String },
    Cancelled { reason: Option<String> },
}

struct PendingMcpOps {
    /// Sender to unblock `captain_notify_human` when the human responds.
    human_reply: Option<tokio::sync::oneshot::Sender<String>>,
    /// Sender to unblock `mate_ask_captain` when the captain steers.
    captain_reply: Option<tokio::sync::oneshot::Sender<String>>,
    /// Sender to unblock `mate_submit` when the captain accepts/steers/cancels.
    mate_review: Option<tokio::sync::oneshot::Sender<MateReviewOutcome>>,
}

impl PendingMcpOps {
    fn new() -> Self {
        Self {
            human_reply: None,
            captain_reply: None,
            mate_review: None,
        }
    }
}

// r[server.multi-repo]
#[derive(Clone)]
pub struct ShipImpl {
    registry: Arc<tokio::sync::Mutex<ProjectRegistry>>,
    agent_discovery: AgentDiscovery,
    agent_driver: Arc<AcpAgentDriver>,
    worktree_ops: Arc<GitWorktreeOps>,
    store: Arc<JsonSessionStore>,
    sessions: Arc<Mutex<HashMap<SessionId, ActiveSession>>>,
    pending_mcp_ops: Arc<Mutex<HashMap<SessionId, PendingMcpOps>>>,
    server_ws_url: Arc<Mutex<String>>,
    startup_started_at: Arc<Mutex<HashMap<SessionId, Instant>>>,
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
            pending_mcp_ops: Arc::new(Mutex::new(HashMap::new())),
            server_ws_url: Arc::new(Mutex::new("ws://127.0.0.1:9/ws".to_owned())),
            startup_started_at: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn set_server_ws_url(&self, url: impl Into<String>) {
        *self
            .server_ws_url
            .lock()
            .expect("server websocket url mutex poisoned") = url.into();
    }

    // r[resilience.server-restart]
    pub async fn load_persisted_sessions(&self) {
        let sessions_list = match self.store.list_sessions().await {
            Ok(list) => list,
            Err(error) => {
                tracing::warn!(%error, "failed to list persisted sessions on startup");
                return;
            }
        };

        let count = sessions_list.len();
        tracing::info!(count, "loading persisted sessions");

        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        for persisted in sessions_list {
            let needs_respawn = persisted
                .current_task
                .as_ref()
                .map(|t| !t.record.status.is_terminal())
                .unwrap_or(false);

            const RESPAWN_MSG: &str = "Server restarted — agents need respawn.";

            let (agent_state, startup_state) = if needs_respawn {
                (
                    AgentState::Error {
                        message: RESPAWN_MSG.into(),
                    },
                    SessionStartupState::Failed {
                        stage: SessionStartupStage::StartingCaptain,
                        message: RESPAWN_MSG.into(),
                    },
                )
            } else {
                (AgentState::Idle, persisted.startup_state.clone())
            };

            let next_event_seq = persisted
                .session_event_log
                .iter()
                .chain(
                    persisted
                        .current_task
                        .as_ref()
                        .into_iter()
                        .flat_map(|t| t.event_log.iter()),
                )
                .map(|e| e.seq.saturating_add(1))
                .max()
                .unwrap_or(0);

            let (events_tx, _) = broadcast::channel(256);
            let session_id = persisted.id.clone();
            let session = ActiveSession {
                id: persisted.id,
                config: persisted.config,
                worktree_path: None,
                captain_handle: None,
                mate_handle: None,
                captain: AgentSnapshot {
                    state: agent_state.clone(),
                    ..persisted.captain
                },
                mate: AgentSnapshot {
                    state: agent_state,
                    ..persisted.mate
                },
                startup_state,
                session_event_log: persisted.session_event_log,
                current_task: persisted.current_task,
                task_history: persisted.task_history,
                captain_block_count: 0,
                mate_block_count: 0,
                pending_permissions: HashMap::new(),
                pending_steer: None,
                events_tx,
                next_event_seq,
            };

            tracing::info!(session_id = %session_id.0, needs_respawn, "loaded persisted session");
            sessions.insert(session_id, session);
        }

        tracing::info!(count, "persisted sessions loaded");
    }

    fn fallback_agent(role: Role, kind: AgentKind) -> AgentSnapshot {
        AgentSnapshot {
            role,
            kind,
            state: AgentState::Error {
                message: "session not found".to_owned(),
            },
            context_remaining_percent: None,
            model_id: None,
            available_models: Vec::new(),
        }
    }

    fn to_session_summary(session: &ActiveSession) -> SessionSummary {
        SessionSummary {
            id: session.id.clone(),
            project: session.config.project.clone(),
            branch_name: session.config.branch_name.clone(),
            captain: session.captain.clone(),
            mate: session.mate.clone(),
            startup_state: session.startup_state.clone(),
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
            startup_state: session.startup_state.clone(),
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
            startup_state: SessionStartupState::Failed {
                stage: SessionStartupStage::ResolvingMcp,
                message: "session not found".to_owned(),
            },
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
            SessionEvent::SessionStartupChanged { .. } => "SessionStartupChanged",
            SessionEvent::TaskStatusChanged { .. } => "TaskStatusChanged",
            SessionEvent::ContextUpdated { .. } => "ContextUpdated",
            SessionEvent::TaskStarted { .. } => "TaskStarted",
            SessionEvent::AgentModelChanged { .. } => "AgentModelChanged",
        }
    }

    fn event_role(event: &SessionEvent) -> Option<Role> {
        match event {
            SessionEvent::BlockAppend { role, .. }
            | SessionEvent::BlockPatch { role, .. }
            | SessionEvent::AgentStateChanged { role, .. }
            | SessionEvent::ContextUpdated { role, .. }
            | SessionEvent::AgentModelChanged { role, .. } => Some(*role),
            SessionEvent::SessionStartupChanged { .. }
            | SessionEvent::TaskStatusChanged { .. }
            | SessionEvent::TaskStarted { .. } => None,
        }
    }

    fn event_block_id(event: &SessionEvent) -> Option<&str> {
        match event {
            SessionEvent::BlockAppend { block_id, .. }
            | SessionEvent::BlockPatch { block_id, .. } => Some(&block_id.0),
            SessionEvent::AgentStateChanged { .. }
            | SessionEvent::SessionStartupChanged { .. }
            | SessionEvent::TaskStatusChanged { .. }
            | SessionEvent::ContextUpdated { .. }
            | SessionEvent::TaskStarted { .. }
            | SessionEvent::AgentModelChanged { .. } => None,
        }
    }

    fn event_task_id(event: &SessionEvent) -> Option<&str> {
        match event {
            SessionEvent::TaskStatusChanged { task_id, .. }
            | SessionEvent::TaskStarted { task_id, .. } => Some(&task_id.0),
            SessionEvent::BlockAppend { .. }
            | SessionEvent::BlockPatch { .. }
            | SessionEvent::AgentStateChanged { .. }
            | SessionEvent::SessionStartupChanged { .. }
            | SessionEvent::ContextUpdated { .. }
            | SessionEvent::AgentModelChanged { .. } => None,
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

    // r[event.replay-complete]
    // r[event.replay.same-events]
    // r[event.replay.followed-by-marker]
    // r[event.replay.per-subscriber]
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

    async fn resolve_project_root(
        &self,
        project: &ProjectName,
    ) -> Result<std::path::PathBuf, String> {
        let registry = self.registry.lock().await;
        registry
            .get(&project.0)
            .map(|project| std::path::PathBuf::from(project.path))
            .ok_or_else(|| format!("project not found: {}", project.0))
    }

    fn startup_message(stage: SessionStartupStage) -> &'static str {
        match stage {
            SessionStartupStage::ResolvingMcp => "Resolving MCP configuration",
            SessionStartupStage::CreatingWorktree => "Creating worktree",
            SessionStartupStage::StartingCaptain => "Starting captain",
            SessionStartupStage::StartingMate => "Starting mate",
            SessionStartupStage::GreetingCaptain => "Greeting user",
        }
    }

    fn startup_elapsed(&self, session_id: &SessionId) -> Duration {
        self.startup_started_at
            .lock()
            .expect("startup timer mutex poisoned")
            .get(session_id)
            .map(Instant::elapsed)
            .unwrap_or_default()
    }

    fn startup_status_text(&self, session_id: &SessionId, stage: SessionStartupStage) -> String {
        let elapsed = self.startup_elapsed(session_id).as_secs_f32();
        format!("{} ({elapsed:.1}s elapsed)", Self::startup_message(stage))
    }

    fn log_startup_step_elapsed(
        &self,
        session_id: &SessionId,
        step: &'static str,
        started_at: Instant,
    ) {
        tracing::info!(
            session_id = %session_id.0,
            step,
            step_elapsed_ms = started_at.elapsed().as_millis(),
            startup_elapsed_ms = self.startup_elapsed(session_id).as_millis(),
            "startup step finished"
        );
    }

    async fn set_startup_stage(
        &self,
        session_id: &SessionId,
        stage: SessionStartupStage,
    ) -> Result<(), String> {
        tracing::info!(
            session_id = %session_id.0,
            ?stage,
            elapsed_ms = self.startup_elapsed(session_id).as_millis(),
            "startup stage updated"
        );
        self.set_startup_state(
            session_id,
            SessionStartupState::Running {
                stage,
                message: self.startup_status_text(session_id, stage),
            },
        )
        .await
    }

    async fn set_startup_state(
        &self,
        session_id: &SessionId,
        state: SessionStartupState,
    ) -> Result<(), String> {
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            apply_event(
                session,
                SessionEvent::SessionStartupChanged {
                    state: state.clone(),
                },
            );
        }

        self.persist_session(session_id).await
    }

    async fn fail_startup(
        &self,
        session_id: &SessionId,
        stage: SessionStartupStage,
        message: String,
    ) {
        let elapsed_ms = self.startup_elapsed(session_id).as_millis();
        Self::log_error("session_startup", &message);
        tracing::warn!(
            session_id = %session_id.0,
            ?stage,
            elapsed_ms,
            %message,
            "session startup failed"
        );
        let _ = self
            .set_startup_state(
                session_id,
                SessionStartupState::Failed {
                    stage,
                    message: format!("{message} ({elapsed_ms}ms elapsed)"),
                },
            )
            .await;
        self.startup_started_at
            .lock()
            .expect("startup timer mutex poisoned")
            .remove(session_id);
    }

    // r[captain.system-prompt]
    fn captain_bootstrap_prompt() -> String {
        [
            "You are the Ship captain for this session.",
            "Act as a senior engineer who scopes work, reviews changes, and delegates to the mate.",
            "Do not write code directly and do not assume there is already a task.",
            "A new session has just been created and there is no active task yet.",
            "Greet the user briefly, explain that you are ready to help plan or review work, then wait.",
            "Do not delegate to the mate yet.",
        ]
        .join("\n")
    }

    // r[captain.tool.transport]
    // r[session.agent.captain]
    async fn install_captain_mcp_server(
        &self,
        session_id: &SessionId,
    ) -> Result<McpServerConfig, String> {
        let command = std::env::current_exe()
            .map_err(|error| format!("failed to locate current executable: {error}"))?;
        let server_ws_url = self
            .server_ws_url
            .lock()
            .expect("server websocket url mutex poisoned")
            .clone();

        Ok(McpServerConfig::Stdio(McpStdioServerConfig {
            name: "ship".to_owned(),
            command: command.display().to_string(),
            args: vec![
                "captain-mcp-server".to_owned(),
                "--session".to_owned(),
                session_id.0.clone(),
                "--server-ws-url".to_owned(),
                server_ws_url,
            ],
            env: Vec::new(),
        }))
    }

    // r[session.agent.mate]
    async fn install_mate_mcp_server(
        &self,
        session_id: &SessionId,
    ) -> Result<McpServerConfig, String> {
        let command = std::env::current_exe()
            .map_err(|error| format!("failed to locate current executable: {error}"))?;
        let server_ws_url = self
            .server_ws_url
            .lock()
            .expect("server websocket url mutex poisoned")
            .clone();

        Ok(McpServerConfig::Stdio(McpStdioServerConfig {
            name: "ship".to_owned(),
            command: command.display().to_string(),
            args: vec![
                "mate-mcp-server".to_owned(),
                "--session".to_owned(),
                session_id.0.clone(),
                "--server-ws-url".to_owned(),
                server_ws_url,
            ],
            env: Vec::new(),
        }))
    }

    pub fn ship_mcp_connection_acceptor(&self) -> ShipMcpConnectionAcceptor {
        ShipMcpConnectionAcceptor { ship: self.clone() }
    }

    async fn append_human_message(
        &self,
        session_id: &SessionId,
        role: Role,
        content: String,
    ) -> Result<(), String> {
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            apply_event(
                session,
                SessionEvent::BlockAppend {
                    block_id: BlockId::new(),
                    role,
                    block: ContentBlock::Text {
                        text: content,
                        source: ship_types::TextSource::Human,
                    },
                },
            );
        }

        self.persist_session(session_id).await
    }

    async fn dispatch_steer_to_mate(
        &self,
        session_id: &SessionId,
        content: String,
    ) -> Result<(), String> {
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let active = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;

            let status = current_task_status(active).map_err(|error| error.to_string())?;
            if status != TaskStatus::Assigned
                && status != TaskStatus::ReviewPending
                && status != TaskStatus::SteerPending
                && status != TaskStatus::Working
            {
                return Err("invalid task transition".to_owned());
            }

            apply_event(
                active,
                SessionEvent::BlockAppend {
                    block_id: BlockId::new(),
                    role: Role::Mate,
                    block: ContentBlock::Text {
                        text: content.clone(),
                        source: ship_types::TextSource::Human,
                    },
                },
            );
            transition_task(active, TaskStatus::Working).map_err(|error| error.to_string())?;
            active.pending_steer = None;
        }

        self.persist_session(session_id).await?;

        let this = self.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            this.prompt_mate_from_steer(session_id, content).await;
        });

        Ok(())
    }

    // r[task.accept]
    async fn accept_task(
        &self,
        session_id: &SessionId,
        summary: Option<String>,
    ) -> Result<(), String> {
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let active = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let status = current_task_status(active).map_err(|error| error.to_string())?;
            if status != TaskStatus::Assigned
                && status != TaskStatus::ReviewPending
                && status != TaskStatus::SteerPending
            {
                return Err("invalid task transition".to_owned());
            }

            if let Some(summary) = summary {
                apply_event(
                    active,
                    SessionEvent::BlockAppend {
                        block_id: BlockId::new(),
                        role: Role::Captain,
                        block: ContentBlock::Text {
                            text: summary,
                            source: ship_types::TextSource::AgentMessage,
                        },
                    },
                );
            }

            transition_task(active, TaskStatus::Accepted).map_err(|error| error.to_string())?;
            archive_terminal_task(active);
        }

        self.persist_session(session_id).await
    }

    // r[task.cancel]
    async fn cancel_task(
        &self,
        session_id: &SessionId,
        reason: Option<String>,
    ) -> Result<(), String> {
        let mate_handle = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let active = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let status = current_task_status(active).map_err(|error| error.to_string())?;
            if status.is_terminal() {
                return Err("session has no active task".to_owned());
            }
            if status == TaskStatus::Working {
                active.mate_handle.clone()
            } else {
                None
            }
        };

        if let Some(mate_handle) = mate_handle
            && let Err(error) = self.agent_driver.cancel(&mate_handle).await
        {
            return Err(error.message);
        }

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let active = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            if let Some(reason) = reason {
                apply_event(
                    active,
                    SessionEvent::BlockAppend {
                        block_id: BlockId::new(),
                        role: Role::Captain,
                        block: ContentBlock::Error { message: reason },
                    },
                );
            }
            transition_task(active, TaskStatus::Cancelled).map_err(|error| error.to_string())?;
            archive_terminal_task(active);
        }

        self.persist_session(session_id).await
    }

    async fn restart_mate(&self, session_id: &SessionId) -> Result<(), String> {
        let (old_handle, mate_kind, worktree_path, extra_servers) = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let worktree_path = session
                .worktree_path
                .clone()
                .ok_or_else(|| "session has no worktree path".to_owned())?;
            (
                session.mate_handle.clone(),
                session.config.mate_kind,
                worktree_path,
                session.config.mcp_servers.clone(),
            )
        };

        if let Some(handle) = old_handle {
            let _ = self.agent_driver.kill(&handle).await;
        }

        let mate_ship_mcp = self.install_mate_mcp_server(session_id).await?;
        let mate_config = AgentSessionConfig {
            worktree_path,
            mcp_servers: {
                let mut servers = extra_servers;
                servers.push(mate_ship_mcp);
                servers
            },
        };

        let (new_handle, model_id, available_models) = self
            .agent_driver
            .spawn(mate_kind, Role::Mate, &mate_config)
            .await
            .map_err(|error| error.message)?;

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(session) = sessions.get_mut(session_id) {
                session.mate_handle = Some(new_handle);
                session.mate.model_id = model_id;
                session.mate.available_models = available_models;
            }
        }

        Ok(())
    }

    // r[task.assign]
    // r[captain.tool.assign]
    async fn captain_tool_assign(
        &self,
        session_id: &SessionId,
        description: String,
        keep: bool,
    ) -> Result<String, String> {
        let task_id = self.start_task(session_id, description.clone()).await?;

        if !keep {
            self.restart_mate(session_id).await?;
        }

        let this = self.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this.dispatch_steer_to_mate(&session_id, description).await {
                Self::log_error("captain_assign dispatch_steer_to_mate", &error);
            }
        });

        Ok(format!("Task {} assigned to the mate.", task_id.0))
    }

    // r[task.steer]
    // r[captain.tool.steer]
    async fn captain_tool_steer(
        &self,
        session_id: &SessionId,
        message: String,
    ) -> Result<String, String> {
        // If the mate is blocked waiting for a reply (mate_ask_captain or mate_submit feedback),
        // resolve that first. Otherwise inject directly into the mate's stream.
        let pending_reply = self
            .pending_mcp_ops
            .lock()
            .expect("pending_mcp_ops mutex poisoned")
            .get_mut(session_id)
            .and_then(|ops| ops.captain_reply.take());

        if let Some(tx) = pending_reply {
            let _ = tx.send(message.clone());
        } else {
            let pending_review = self
                .pending_mcp_ops
                .lock()
                .expect("pending_mcp_ops mutex poisoned")
                .get_mut(session_id)
                .and_then(|ops| ops.mate_review.take());

            if let Some(tx) = pending_review {
                let _ = tx.send(MateReviewOutcome::Feedback {
                    message: message.clone(),
                });
            } else {
                self.dispatch_steer_to_mate(session_id, message.clone())
                    .await?;
            }
        }

        Ok("Steer sent to the mate.".to_owned())
    }

    // r[captain.tool.accept]
    async fn captain_tool_accept(
        &self,
        session_id: &SessionId,
        summary: Option<String>,
    ) -> Result<String, String> {
        let pending_review = self
            .pending_mcp_ops
            .lock()
            .expect("pending_mcp_ops mutex poisoned")
            .get_mut(session_id)
            .and_then(|ops| ops.mate_review.take());

        if let Some(tx) = pending_review {
            let _ = tx.send(MateReviewOutcome::Accepted {
                summary: summary.clone(),
            });
        }

        self.accept_task(session_id, summary.clone()).await?;
        Ok("Accepted the active task.".to_owned())
    }

    async fn captain_tool_cancel(
        &self,
        session_id: &SessionId,
        reason: Option<String>,
    ) -> Result<String, String> {
        let pending_review = self
            .pending_mcp_ops
            .lock()
            .expect("pending_mcp_ops mutex poisoned")
            .get_mut(session_id)
            .and_then(|ops| ops.mate_review.take());

        if let Some(tx) = pending_review {
            let _ = tx.send(MateReviewOutcome::Cancelled {
                reason: reason.clone(),
            });
        }

        self.cancel_task(session_id, reason.clone()).await?;
        Ok("Task cancelled.".to_owned())
    }

    async fn captain_tool_notify_human(
        &self,
        session_id: &SessionId,
        _message: String,
    ) -> Result<String, String> {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        {
            let mut ops = self
                .pending_mcp_ops
                .lock()
                .expect("pending_mcp_ops mutex poisoned");
            let entry = ops
                .entry(session_id.clone())
                .or_insert_with(PendingMcpOps::new);
            entry.human_reply = Some(tx);
        }

        match rx.await {
            Ok(reply) => Ok(reply),
            Err(_) => Err("human reply channel closed".to_owned()),
        }
    }

    async fn mate_tool_send_update(
        &self,
        session_id: &SessionId,
        message: String,
    ) -> Result<String, String> {
        // Inject the update into the captain's stream as a user message, then prompt the captain.
        let injected = format!("The mate sent you an update: {message}");
        self.append_human_message(session_id, Role::Captain, injected.clone())
            .await?;

        let this = self.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this
                .prompt_agent(&session_id, Role::Captain, injected)
                .await
            {
                Self::log_error("mate_send_update prompt_captain", &error);
            }
        });

        Ok("Update sent to the captain.".to_owned())
    }

    async fn mate_tool_ask_captain(
        &self,
        session_id: &SessionId,
        question: String,
    ) -> Result<String, String> {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        {
            let mut ops = self
                .pending_mcp_ops
                .lock()
                .expect("pending_mcp_ops mutex poisoned");
            let entry = ops
                .entry(session_id.clone())
                .or_insert_with(PendingMcpOps::new);
            entry.captain_reply = Some(tx);
        }

        let injected = format!("The mate has a question for you: {question}");
        self.append_human_message(session_id, Role::Captain, injected.clone())
            .await?;

        let this = self.clone();
        let session_id_clone = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this
                .prompt_agent(&session_id_clone, Role::Captain, injected)
                .await
            {
                Self::log_error("mate_ask_captain prompt_captain", &error);
            }
        });

        match rx.await {
            Ok(reply) => Ok(reply),
            Err(_) => Err("captain reply channel closed".to_owned()),
        }
    }

    async fn mate_tool_submit(
        &self,
        session_id: &SessionId,
        summary: String,
    ) -> Result<String, String> {
        let (tx, rx) = tokio::sync::oneshot::channel::<MateReviewOutcome>();
        {
            let mut ops = self
                .pending_mcp_ops
                .lock()
                .expect("pending_mcp_ops mutex poisoned");
            let entry = ops
                .entry(session_id.clone())
                .or_insert_with(PendingMcpOps::new);
            entry.mate_review = Some(tx);
        }

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(active) = sessions.get_mut(session_id) {
                let _ = transition_task(active, TaskStatus::ReviewPending);
            }
        }
        self.persist_session(session_id).await?;

        let injected = format!("The mate has submitted their work for review: {summary}");
        self.append_human_message(session_id, Role::Captain, injected.clone())
            .await?;

        let this = self.clone();
        let session_id_clone = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this
                .prompt_agent(&session_id_clone, Role::Captain, injected)
                .await
            {
                Self::log_error("mate_submit prompt_captain", &error);
            }
        });

        match rx.await {
            Ok(MateReviewOutcome::Accepted { summary }) => Ok(format!(
                "Work accepted. {}",
                summary.as_deref().unwrap_or("")
            )),
            Ok(MateReviewOutcome::Feedback { message }) => {
                Ok(format!("Captain feedback (please revise): {message}"))
            }
            Ok(MateReviewOutcome::Cancelled { reason }) => Err(format!(
                "Task cancelled: {}",
                reason.as_deref().unwrap_or("no reason given")
            )),
            Err(_) => Err("review channel closed".to_owned()),
        }
    }

    async fn start_session_runtime(&self, session_id: SessionId) {
        let stage = SessionStartupStage::ResolvingMcp;
        let _ = self.set_startup_stage(&session_id, stage).await;

        let step_started_at = Instant::now();
        let (project, base_branch, resolved_mcp_servers) = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(session) = sessions.get(&session_id) else {
                return;
            };
            (
                session.config.project.clone(),
                session.config.base_branch.clone(),
                session.config.mcp_servers.clone(),
            )
        };
        self.log_startup_step_elapsed(&session_id, "read-session-config", step_started_at);

        let step_started_at = Instant::now();
        let repo_root = match self.resolve_project_root(&project).await {
            Ok(value) => value,
            Err(error) => {
                self.fail_startup(&session_id, stage, error).await;
                return;
            }
        };
        self.log_startup_step_elapsed(&session_id, "resolve-project-root", step_started_at);

        let stage = SessionStartupStage::CreatingWorktree;
        let _ = self.set_startup_stage(&session_id, stage).await;
        let step_started_at = Instant::now();
        let worktree_path = match self
            .worktree_ops
            .create_worktree(&session_id, &base_branch, "session", &repo_root)
            .await
        {
            Ok(path) => path,
            Err(error) => {
                self.fail_startup(&session_id, stage, error.message).await;
                return;
            }
        };
        self.log_startup_step_elapsed(&session_id, "create-worktree", step_started_at);
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(session) = sessions.get_mut(&session_id) {
                session.worktree_path = Some(worktree_path.clone());
            }
        }
        let _ = self.persist_session(&session_id).await;

        let step_started_at = Instant::now();
        let (captain_ship_mcp, mate_ship_mcp) = match tokio::join!(
            self.install_captain_mcp_server(&session_id),
            self.install_mate_mcp_server(&session_id),
        ) {
            (Ok(c), Ok(m)) => (c, m),
            (Err(error), _) | (_, Err(error)) => {
                self.fail_startup(&session_id, SessionStartupStage::StartingCaptain, error)
                    .await;
                return;
            }
        };
        self.log_startup_step_elapsed(&session_id, "install-mcp-servers", step_started_at);

        self.pending_mcp_ops
            .lock()
            .expect("pending_mcp_ops mutex poisoned")
            .insert(session_id.clone(), PendingMcpOps::new());

        let stage = SessionStartupStage::StartingCaptain;
        let _ = self.set_startup_stage(&session_id, stage).await;
        let (captain_kind, mate_kind) = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get(&session_id).expect("session exists");
            (session.config.captain_kind, session.config.mate_kind)
        };
        let captain_config = AgentSessionConfig {
            worktree_path: worktree_path.clone(),
            mcp_servers: {
                let mut servers = resolved_mcp_servers.clone();
                servers.push(captain_ship_mcp);
                servers
            },
        };
        let mate_config = AgentSessionConfig {
            worktree_path: worktree_path.clone(),
            mcp_servers: {
                let mut servers = resolved_mcp_servers.clone();
                servers.push(mate_ship_mcp);
                servers
            },
        };
        let captain_started_at = Instant::now();
        let mate_started_at = Instant::now();
        let (captain_result, mate_result) = tokio::join!(
            self.agent_driver
                .spawn(captain_kind, Role::Captain, &captain_config),
            self.agent_driver.spawn(mate_kind, Role::Mate, &mate_config),
        );
        let (captain_handle, captain_model_id, captain_available_models) = match captain_result {
            Ok(result) => {
                self.log_startup_step_elapsed(&session_id, "spawn-captain", captain_started_at);
                result
            }
            Err(error) => {
                if let Ok((mate_handle, _, _)) = mate_result {
                    let _ = self.agent_driver.kill(&mate_handle).await;
                }
                self.fail_startup(&session_id, stage, error.message).await;
                return;
            }
        };
        let (mate_handle, mate_model_id, mate_available_models) = match mate_result {
            Ok(result) => {
                self.log_startup_step_elapsed(&session_id, "spawn-mate", mate_started_at);
                result
            }
            Err(error) => {
                let _ = self.agent_driver.kill(&captain_handle).await;
                self.fail_startup(
                    &session_id,
                    SessionStartupStage::StartingMate,
                    error.message,
                )
                .await;
                return;
            }
        };
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(session) = sessions.get_mut(&session_id) {
                session.captain_handle = Some(captain_handle);
                session.mate_handle = Some(mate_handle);
                apply_event(
                    session,
                    ship_types::SessionEvent::AgentModelChanged {
                        role: Role::Captain,
                        model_id: captain_model_id,
                        available_models: captain_available_models,
                    },
                );
                apply_event(
                    session,
                    ship_types::SessionEvent::AgentModelChanged {
                        role: Role::Mate,
                        model_id: mate_model_id,
                        available_models: mate_available_models,
                    },
                );
            }
        }
        let _ = self.persist_session(&session_id).await;

        let stage = SessionStartupStage::GreetingCaptain;
        let _ = self.set_startup_stage(&session_id, stage).await;
        let step_started_at = Instant::now();
        if let Err(error) = self
            .prompt_agent(&session_id, Role::Captain, Self::captain_bootstrap_prompt())
            .await
        {
            Self::log_error("startup_prompt_captain", &error);
            self.fail_startup(&session_id, stage, error).await;
            return;
        }
        self.log_startup_step_elapsed(&session_id, "greet-captain", step_started_at);
        let _ = self
            .set_startup_state(&session_id, SessionStartupState::Ready)
            .await;

        self.startup_started_at
            .lock()
            .expect("startup timer mutex poisoned")
            .remove(&session_id);
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
        let worktree_path = session
            .worktree_path
            .as_ref()
            .ok_or_else(|| "session worktree not ready".to_owned())?;
        let repo_root = Self::repo_root_for_worktree(worktree_path)?;

        if let Some(handle) = &session.captain_handle
            && let Err(error) = self.agent_driver.kill(handle).await
        {
            Self::log_error("close_session_kill_captain", &error.message);
        }
        if let Some(handle) = &session.mate_handle
            && let Err(error) = self.agent_driver.kill(handle).await
        {
            Self::log_error("close_session_kill_mate", &error.message);
        }

        if worktree_path.exists() {
            self.worktree_ops
                .remove_worktree(worktree_path, force)
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
                startup_state: session.startup_state.clone(),
                session_event_log: session.session_event_log.clone(),
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
            .ok_or_else(|| format!("{role:?} agent not ready"))?
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
            .ok_or_else(|| format!("{role:?} agent not ready"))?
        };

        self.persist_session(session_id).await?;

        let (stop_tx, pump_handle) = self.spawn_notification_pump(session_id.clone(), role);
        let response = match self.agent_driver.prompt(&handle, &prompt).await {
            Ok(response) => response,
            Err(error) => {
                let message = error.message;
                let _ = stop_tx.send(());
                let _ = pump_handle.await;
                {
                    let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    if let Some(session) = sessions.get_mut(session_id) {
                        set_agent_state(
                            session,
                            role,
                            AgentState::Error {
                                message: message.clone(),
                            },
                        );
                    }
                }
                let _ = self.persist_session(session_id).await;
                return Err(message);
            }
        };
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

    // r[acp.stop-reason]
    async fn handle_mate_stop_reason(
        &self,
        session_id: &SessionId,
        stop_reason: ship_core::StopReason,
    ) -> Result<(), String> {
        match stop_reason {
            ship_core::StopReason::EndTurn => {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let session = sessions
                    .get_mut(session_id)
                    .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                transition_task(session, TaskStatus::ReviewPending)
                    .map_err(|error| error.to_string())?;
            }
            ship_core::StopReason::Cancelled => {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let session = sessions
                    .get_mut(session_id)
                    .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                transition_task(session, TaskStatus::Cancelled)
                    .map_err(|error| error.to_string())?;
                archive_terminal_task(session);
            }
            ship_core::StopReason::ContextExhausted => {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let session = sessions
                    .get_mut(session_id)
                    .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                set_agent_state(session, Role::Mate, AgentState::ContextExhausted);
            }
        }

        self.persist_session(session_id).await
    }

    // r[session.single-task]
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
            if session.startup_state != SessionStartupState::Ready {
                return Err("session startup is not complete".to_owned());
            }
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

    // r[session.list]
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

    // r[session.create]
    async fn create_session(&self, req: CreateSessionRequest) -> CreateSessionResponse {
        let project_exists = {
            let registry = self.registry.lock().await;
            registry.get(&req.project.0).is_some()
        };
        if !project_exists {
            return CreateSessionResponse::Failed {
                message: format!("project not found: {}", req.project.0),
            };
        }

        let effective_mcp_servers = match self
            .resolve_session_mcp_servers(&req.project, req.mcp_servers.clone())
            .await
        {
            Ok((_, mcp_servers)) => mcp_servers,
            Err(error) => {
                Self::log_error("resolve_session_mcp_servers", &error);
                return CreateSessionResponse::Failed { message: error };
            }
        };

        let session_id = SessionId::new();
        let branch_name = format!("ship/{}/session", short_id(&session_id));
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
                mcp_servers: effective_mcp_servers,
            },
            worktree_path: None,
            captain_handle: None,
            mate_handle: None,
            captain: AgentSnapshot {
                role: Role::Captain,
                kind: req.captain_kind,
                state: AgentState::Idle,
                context_remaining_percent: None,
                model_id: None,
                available_models: Vec::new(),
            },
            mate: AgentSnapshot {
                role: Role::Mate,
                kind: req.mate_kind,
                state: AgentState::Idle,
                context_remaining_percent: None,
                model_id: None,
                available_models: Vec::new(),
            },
            startup_state: SessionStartupState::Pending,
            session_event_log: Vec::new(),
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
        if let Err(error) = self.persist_session(&session_id).await {
            self.sessions
                .lock()
                .expect("sessions mutex poisoned")
                .remove(&session_id);
            return CreateSessionResponse::Failed { message: error };
        }

        self.startup_started_at
            .lock()
            .expect("startup timer mutex poisoned")
            .insert(session_id.clone(), Instant::now());

        let this = self.clone();
        let startup_session_id = session_id.clone();
        tokio::spawn(async move {
            this.start_session_runtime(startup_session_id).await;
        });

        CreateSessionResponse::Created { session_id }
    }

    async fn steer(&self, session: SessionId, content: String) {
        if let Err(error) = self.dispatch_steer_to_mate(&session, content).await {
            Self::log_error("steer", &error);
        }
    }

    // r[acp.prompt]
    async fn prompt_captain(&self, session: SessionId, content: String) {
        if let Err(error) = self
            .append_human_message(&session, Role::Captain, content.clone())
            .await
        {
            Self::log_error("prompt_captain_append_human_message", &error);
            return;
        }

        let this = self.clone();
        tokio::spawn(async move {
            if let Err(error) = this.prompt_agent(&session, Role::Captain, content).await {
                Self::log_error("prompt_captain", &error);
            }
        });
    }

    async fn accept(&self, session: SessionId) {
        if let Err(error) = self.accept_task(&session, None).await {
            Self::log_error("accept", &error);
        }
    }

    async fn cancel(&self, session: SessionId) {
        if let Err(error) = self.cancel_task(&session, None).await {
            Self::log_error("cancel", &error);
        }
    }

    async fn resolve_permission(
        &self,
        session: SessionId,
        permission_id: String,
        option_id: String,
    ) {
        let (pending, handle) = {
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
            let Some(handle) = handle else {
                Self::log_error("resolve_permission", "agent not ready");
                return;
            };
            (pending.clone(), handle)
        };

        if let Err(error) = self
            .agent_driver
            .resolve_permission(&handle, &permission_id, &option_id)
            .await
        {
            Self::log_error("resolve_permission", &error.message);
            return;
        }

        let resolution = pending
            .request
            .options
            .as_ref()
            .and_then(|options| options.iter().find(|option| option.option_id == option_id))
            .map(|option| match option.kind {
                ship_types::PermissionOptionKind::AllowOnce
                | ship_types::PermissionOptionKind::AllowAlways => {
                    ship_types::PermissionResolution::Approved
                }
                ship_types::PermissionOptionKind::RejectOnce
                | ship_types::PermissionOptionKind::RejectAlways
                | ship_types::PermissionOptionKind::Other => {
                    ship_types::PermissionResolution::Denied
                }
            });
        let Some(resolution) = resolution else {
            Self::log_error("resolve_permission", "permission option not found");
            return;
        };

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(active) = sessions.get_mut(&session) else {
                Self::log_error("resolve_permission", "session missing after driver call");
                return;
            };

            set_agent_state(
                active,
                pending.role,
                AgentState::Working {
                    plan: None,
                    activity: Some("Permission resolved".to_owned()),
                },
            );
            apply_event(
                active,
                SessionEvent::BlockPatch {
                    block_id: pending.block_id,
                    role: pending.role,
                    patch: ship_types::BlockPatch::PermissionResolve { resolution },
                },
            );
        }

        if let Err(error) = self.persist_session(&session).await {
            Self::log_error("resolve_permission_persist", &error);
        }
    }

    async fn retry_agent(&self, _session: SessionId, _role: Role) {}

    async fn set_agent_model(
        &self,
        session: SessionId,
        role: Role,
        model_id: String,
    ) -> SetAgentModelResponse {
        let agent_driver = self.agent_driver.clone();
        let sessions = self.sessions.clone();

        let handle = {
            let sessions = sessions.lock().expect("sessions mutex poisoned");
            let Some(session_state) = sessions.get(&session) else {
                return SetAgentModelResponse::SessionNotFound;
            };
            match role {
                Role::Captain => session_state.captain_handle.clone(),
                Role::Mate => session_state.mate_handle.clone(),
            }
        };

        let Some(handle) = handle else {
            return SetAgentModelResponse::AgentNotSpawned;
        };

        match agent_driver.set_model(&handle, &model_id).await {
            Ok(()) => {
                let mut sessions = sessions.lock().expect("sessions mutex poisoned");
                if let Some(session_state) = sessions.get_mut(&session) {
                    let available_models = match role {
                        Role::Captain => session_state.captain.available_models.clone(),
                        Role::Mate => session_state.mate.available_models.clone(),
                    };
                    apply_event(
                        session_state,
                        ship_types::SessionEvent::AgentModelChanged {
                            role,
                            model_id: Some(model_id),
                            available_models,
                        },
                    );
                }
                SetAgentModelResponse::Ok
            }
            Err(error) => SetAgentModelResponse::Failed {
                message: error.message,
            },
        }
    }

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
            if let Some(worktree_path) = worktree_path.as_ref()
                && worktree_path.exists()
            {
                self.worktree_ops
                    .has_uncommitted_changes(worktree_path)
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

    // r[event.subscribe.replay]
    // r[event.subscribe.roam-channel]
    // r[sharing.event-broadcast]
    async fn subscribe_events(&self, session: SessionId, output: Tx<SubscribeMessage>) {
        tracing::info!(session_id = %session.0, "subscriber connected");
        let session_data = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            sessions.get(&session).map(|active| {
                let replay = active
                    .session_event_log
                    .iter()
                    .cloned()
                    .chain(
                        active
                            .current_task
                            .as_ref()
                            .into_iter()
                            .flat_map(|task| task.event_log.clone()),
                    )
                    .collect::<Vec<_>>();
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

pub struct ShipMcpConnectionAcceptor {
    ship: ShipImpl,
}

impl ConnectionAcceptor for ShipMcpConnectionAcceptor {
    fn accept(
        &self,
        _conn_id: roam::ConnectionId,
        peer_settings: &ConnectionSettings,
        metadata: &[MetadataEntry],
    ) -> Result<AcceptedConnection, Metadata<'static>> {
        let Some(service_name) = metadata_string(metadata, "ship-service") else {
            return Err(rejection_metadata("missing ship-service metadata"));
        };
        let Some(session_id) = metadata_string(metadata, "ship-session-id") else {
            return Err(rejection_metadata("missing ship-session-id metadata"));
        };
        let session_id = SessionId(session_id.to_owned());
        if !self
            .ship
            .sessions
            .lock()
            .expect("sessions mutex poisoned")
            .contains_key(&session_id)
        {
            return Err(rejection_metadata("unknown session"));
        }

        let ship = self.ship.clone();
        let settings = ConnectionSettings {
            parity: peer_settings.parity.other(),
            max_concurrent_requests: 64,
        };

        match service_name {
            "captain-mcp" => Ok(AcceptedConnection {
                settings,
                metadata: Vec::new(),
                setup: Box::new(move |connection| {
                    tokio::spawn(async move {
                        let mut driver = Driver::new(
                            connection,
                            CaptainMcpDispatcher::new(CaptainMcpSessionService {
                                ship,
                                session_id,
                            }),
                        );
                        driver.run().await;
                    });
                }),
            }),
            "mate-mcp" => Ok(AcceptedConnection {
                settings,
                metadata: Vec::new(),
                setup: Box::new(move |connection| {
                    tokio::spawn(async move {
                        let mut driver = Driver::new(
                            connection,
                            MateMcpDispatcher::new(MateMcpSessionService { ship, session_id }),
                        );
                        driver.run().await;
                    });
                }),
            }),
            _ => Err(rejection_metadata("unknown ship-service")),
        }
    }
}

#[derive(Clone)]
struct CaptainMcpSessionService {
    ship: ShipImpl,
    session_id: SessionId,
}

impl CaptainMcpSessionService {
    fn response(result: Result<String, String>) -> McpToolCallResponse {
        match result {
            Ok(text) => McpToolCallResponse {
                text,
                is_error: false,
            },
            Err(text) => McpToolCallResponse {
                text,
                is_error: true,
            },
        }
    }
}

impl CaptainMcp for CaptainMcpSessionService {
    // r[captain.tool.assign]
    async fn captain_assign(&self, description: String, keep: bool) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_assign(&self.session_id, description, keep)
                .await,
        )
    }

    // r[captain.tool.steer]
    async fn captain_steer(&self, message: String) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_steer(&self.session_id, message)
                .await,
        )
    }

    // r[captain.tool.accept]
    async fn captain_accept(&self, summary: Option<String>) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_accept(&self.session_id, summary)
                .await,
        )
    }

    // r[captain.tool.cancel]
    async fn captain_cancel(&self, reason: Option<String>) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_cancel(&self.session_id, reason)
                .await,
        )
    }

    // r[captain.tool.notify-human]
    async fn captain_notify_human(&self, message: String) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_notify_human(&self.session_id, message)
                .await,
        )
    }
}

#[derive(Clone)]
struct MateMcpSessionService {
    ship: ShipImpl,
    session_id: SessionId,
}

impl MateMcpSessionService {
    fn response(result: Result<String, String>) -> McpToolCallResponse {
        match result {
            Ok(text) => McpToolCallResponse {
                text,
                is_error: false,
            },
            Err(text) => McpToolCallResponse {
                text,
                is_error: true,
            },
        }
    }
}

impl MateMcp for MateMcpSessionService {
    // r[mate.tool.send-update]
    async fn mate_send_update(&self, message: String) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_send_update(&self.session_id, message)
                .await,
        )
    }

    // r[mate.tool.ask-captain]
    async fn mate_ask_captain(&self, question: String) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_ask_captain(&self.session_id, question)
                .await,
        )
    }

    // r[mate.tool.submit]
    async fn mate_submit(&self, summary: String) -> McpToolCallResponse {
        Self::response(self.ship.mate_tool_submit(&self.session_id, summary).await)
    }
}

fn metadata_string<'a>(metadata: &'a [MetadataEntry], key: &str) -> Option<&'a str> {
    metadata.iter().find_map(|entry| {
        if entry.key != key {
            return None;
        }
        match entry.value {
            MetadataValue::String(value) => Some(value),
            _ => None,
        }
    })
}

fn rejection_metadata(reason: &'static str) -> Metadata<'static> {
    vec![MetadataEntry {
        key: "reason",
        value: MetadataValue::String(reason),
        flags: MetadataFlags::NONE,
    }]
}

fn short_id(id: &SessionId) -> String {
    id.0.to_string().chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use ship_core::{ProjectRegistry, SessionStore};
    use ship_service::Ship;
    use ship_types::{
        AgentDiscovery, AgentKind, CreateSessionRequest, CreateSessionResponse, CurrentTask,
        McpServerConfig, McpStdioServerConfig, ProjectName, SessionEvent, SessionEventEnvelope,
        SessionId, SessionStartupState, SubscribeMessage, TaskId, TaskRecord, TaskStatus,
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

    async fn create_session_for_workflow_test(test_name: &str) -> (PathBuf, ShipImpl, SessionId) {
        let dir = make_temp_dir(test_name);
        let config_dir = dir.join("config");
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join(".ship")).expect("project ship dir should exist");

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
                mcp_servers: None,
            },
        )
        .await;

        let session_id = match response {
            CreateSessionResponse::Created { session_id } => session_id,
            CreateSessionResponse::Failed { message } => {
                panic!("create session should succeed: {message}")
            }
        };

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.startup_state = SessionStartupState::Ready;
            session.current_task = Some(CurrentTask {
                record: TaskRecord {
                    id: TaskId::new(),
                    description: "Investigate workflow".to_owned(),
                    status: TaskStatus::Assigned,
                },
                content_history: Vec::new(),
                event_log: Vec::new(),
            });
        }

        (dir, ship, session_id)
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
                mcp_servers: None,
            },
        )
        .await;

        assert!(matches!(response, CreateSessionResponse::Failed { .. }));

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify acp.mcp.config]
    // r[verify acp.mcp.defaults]
    // r[verify project.mcp-defaults]
    #[tokio::test]
    async fn create_session_snapshots_effective_mcp_defaults_into_session_config() {
        let dir = make_temp_dir("create-session-snapshots-mcp-defaults");
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
                mcp_servers: None,
            },
        )
        .await;

        let session_id = match response {
            CreateSessionResponse::Created { session_id } => session_id,
            CreateSessionResponse::Failed { message } => {
                panic!("create session should succeed: {message}")
            }
        };

        let session = ship
            .store
            .load_session(&session_id)
            .await
            .expect("session store should load")
            .expect("session should be persisted");
        assert_eq!(
            session.config.mcp_servers,
            vec![McpServerConfig::Stdio(McpStdioServerConfig {
                name: "project".to_owned(),
                command: "/usr/bin/project-mcp".to_owned(),
                args: Vec::new(),
                env: Vec::new(),
            })]
        );

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

    // r[verify captain.tool.steer]
    #[tokio::test]
    async fn captain_tool_steer_dispatches_directly_to_mate() {
        let (dir, ship, session_id) =
            create_session_for_workflow_test("captain-tool-steer-direct").await;

        let result = ship
            .captain_tool_steer(&session_id, "Ask the mate to add coverage".to_owned())
            .await
            .expect("captain tool should succeed");

        assert_eq!(result, "Steer sent to the mate.");

        let detail = Ship::get_session(&ship, session_id.clone()).await;
        assert_eq!(
            detail
                .current_task
                .as_ref()
                .expect("task should exist")
                .status,
            TaskStatus::Working
        );
        assert_eq!(detail.pending_steer, None);

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify proto.steer]
    #[tokio::test]
    async fn explicit_steer_dispatches_pending_review_to_the_mate_path() {
        let (dir, ship, session_id) =
            create_session_for_workflow_test("explicit-steer-dispatch").await;

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session
                .current_task
                .as_mut()
                .expect("task should exist")
                .record
                .status = TaskStatus::SteerPending;
            session.pending_steer = Some("Old captain steer".to_owned());
        }

        ship.dispatch_steer_to_mate(&session_id, "Send the approved steer".to_owned())
            .await
            .expect("explicit steer should dispatch");

        let detail = Ship::get_session(&ship, session_id.clone()).await;
        assert_eq!(
            detail
                .current_task
                .as_ref()
                .expect("task should exist")
                .status,
            TaskStatus::Working
        );
        assert!(detail.pending_steer.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }
}
