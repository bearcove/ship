use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use roam::Tx;
use serde_json::{Value, json};
use ship_core::{
    AcpAgentDriver, ActiveSession, AgentDriver, AgentSessionConfig, GitWorktreeOps,
    JsonSessionStore, ProjectRegistry, SessionStore, WorktreeOps, apply_event,
    archive_terminal_task, current_task_status, rebuild_materialized_from_event_log,
    resolve_mcp_servers, set_agent_state, transition_task,
};
use ship_service::Ship;
use ship_types::{
    AgentDiscovery, AgentKind, AgentSnapshot, AgentState, AssignTaskResponse, AutonomyMode,
    BlockId, CloseSessionRequest, CloseSessionResponse, ContentBlock, CreateSessionRequest,
    CreateSessionResponse, CurrentTask, McpServerConfig, McpStdioServerConfig, PersistedSession,
    ProjectInfo, ProjectName, Role, SessionConfig, SessionDetail, SessionEvent, SessionId,
    SessionStartupStage, SessionStartupState, SessionSummary, SubscribeMessage, TaskId, TaskRecord,
    TaskStatus, ToolCallContent, ToolCallStatus,
};
use tokio::net::UnixListener;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::captain_mcp::{ToolDefinition, ToolHandler, ToolResult};

struct CaptainMcpHandle {
    socket_path: PathBuf,
    task: JoinHandle<()>,
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
    captain_mcp: Arc<Mutex<HashMap<SessionId, CaptainMcpHandle>>>,
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
            captain_mcp: Arc::new(Mutex::new(HashMap::new())),
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
        }
    }

    fn event_role(event: &SessionEvent) -> Option<Role> {
        match event {
            SessionEvent::BlockAppend { role, .. }
            | SessionEvent::BlockPatch { role, .. }
            | SessionEvent::AgentStateChanged { role, .. }
            | SessionEvent::ContextUpdated { role, .. } => Some(*role),
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
            | SessionEvent::SessionStartupChanged { .. }
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
        Self::log_error("session_startup", &message);
        let _ = self
            .set_startup_state(
                session_id,
                SessionStartupState::Failed {
                    stage,
                    message: message.clone(),
                },
            )
            .await;
    }

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

    fn captain_assignment_prompt(description: &str) -> String {
        format!(
            "The human assigned a new task:\n{description}\n\nReview it as the captain. You may ask a clarifying question in plain text, or call Ship MCP tools when you want to delegate, accept, or reject the task. Do not write code directly."
        )
    }

    fn captain_mcp_tools() -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "ship_steer",
                description: "Delegate work on the active task to the mate.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"],
                    "additionalProperties": false,
                }),
            },
            ToolDefinition {
                name: "ship_accept",
                description: "Accept the active task when it is complete.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "summary": { "type": "string" }
                    },
                    "additionalProperties": false,
                }),
            },
            ToolDefinition {
                name: "ship_reject",
                description: "Reject the active task and cancel the current approach.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "reason": { "type": "string" },
                        "message": { "type": "string" }
                    },
                    "required": ["reason"],
                    "additionalProperties": false,
                }),
            },
        ]
    }

    async fn install_captain_mcp_server(
        &self,
        session_id: &SessionId,
        worktree_path: &std::path::Path,
    ) -> Result<McpServerConfig, String> {
        let socket_path = worktree_path.join(".ship-captain-mcp.sock");
        match std::fs::remove_file(&socket_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("failed to clear captain MCP socket: {error}")),
        }

        let listener = UnixListener::bind(&socket_path)
            .map_err(|error| format!("failed to bind captain MCP socket: {error}"))?;

        let session_id_clone = session_id.clone();
        let this = self.clone();
        let handler: ToolHandler = Arc::new(move |name, arguments| {
            let this = this.clone();
            let session_id = session_id_clone.clone();
            Box::pin(async move {
                this.handle_captain_mcp_call(&session_id, &name, arguments)
                    .await
            })
        });

        let task = tokio::spawn(async move {
            crate::captain_mcp::serve(listener, Self::captain_mcp_tools(), handler).await;
        });

        self.captain_mcp
            .lock()
            .expect("captain mcp mutex poisoned")
            .insert(
                session_id.clone(),
                CaptainMcpHandle {
                    socket_path: socket_path.clone(),
                    task,
                },
            );

        let command = std::env::current_exe()
            .map_err(|error| format!("failed to resolve ship executable: {error}"))?;

        Ok(McpServerConfig::Stdio(McpStdioServerConfig {
            name: "ship".to_owned(),
            command: command.display().to_string(),
            args: vec![
                "captain-mcp-proxy".to_owned(),
                "--socket".to_owned(),
                socket_path.display().to_string(),
            ],
            env: Vec::new(),
        }))
    }

    fn clear_captain_mcp_server(&self, session_id: &SessionId) {
        if let Some(handle) = self
            .captain_mcp
            .lock()
            .expect("captain mcp mutex poisoned")
            .remove(session_id)
        {
            handle.task.abort();
            let _ = std::fs::remove_file(handle.socket_path);
        }
    }

    async fn handle_captain_mcp_call(
        &self,
        session_id: &SessionId,
        name: &str,
        arguments: Value,
    ) -> ToolResult {
        let outcome = match name {
            "ship_steer" => {
                let Some(message) = arguments.get("message").and_then(Value::as_str) else {
                    return ToolResult {
                        text: "missing required argument: message".to_owned(),
                        is_error: true,
                    };
                };
                self.captain_tool_steer(session_id, message.to_owned())
                    .await
            }
            "ship_accept" => {
                let summary = arguments
                    .get("summary")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.captain_tool_accept(session_id, summary).await
            }
            "ship_reject" => {
                let Some(reason) = arguments.get("reason").and_then(Value::as_str) else {
                    return ToolResult {
                        text: "missing required argument: reason".to_owned(),
                        is_error: true,
                    };
                };
                let message = arguments
                    .get("message")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.captain_tool_reject(session_id, reason.to_owned(), message)
                    .await
            }
            other => Err(format!("unknown tool: {other}")),
        };

        match outcome {
            Ok(text) => ToolResult {
                text,
                is_error: false,
            },
            Err(text) => ToolResult {
                text,
                is_error: true,
            },
        }
    }

    async fn append_captain_tool_call(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        arguments: String,
        content: Vec<ToolCallContent>,
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
                    role: Role::Captain,
                    block: ContentBlock::ToolCall {
                        tool_call_id: None,
                        tool_name: tool_name.to_owned(),
                        arguments,
                        kind: Some(ship_types::ToolCallKind::Other),
                        target: Some(ship_types::ToolTarget::None),
                        raw_input: None,
                        raw_output: None,
                        locations: Vec::new(),
                        status: ToolCallStatus::Success,
                        content,
                        error: None,
                    },
                },
            );
        }
        self.persist_session(session_id).await
    }

    async fn queue_captain_steer_for_review(
        &self,
        session_id: &SessionId,
        content: String,
    ) -> Result<(), String> {
        let mut replaced_pending_steer = false;
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let active = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;

            let status = current_task_status(active).map_err(|error| error.to_string())?;
            if status != TaskStatus::Assigned && status != TaskStatus::ReviewPending {
                if status == TaskStatus::SteerPending {
                    active.pending_steer = Some(content.clone());
                    replaced_pending_steer = true;
                } else {
                    return Err("invalid task transition".to_owned());
                }
            }

            if !replaced_pending_steer {
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

                transition_task(active, TaskStatus::SteerPending)
                    .map_err(|error| error.to_string())?;
                active.pending_steer = Some(content);
            }
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
            {
                return Err("invalid task transition".to_owned());
            }

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

    async fn captain_delegate_to_mate(
        &self,
        session_id: &SessionId,
        content: String,
    ) -> Result<AutonomyMode, String> {
        let mode = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?
                .config
                .autonomy_mode
        };

        if mode == AutonomyMode::Autonomous {
            self.dispatch_steer_to_mate(session_id, content).await?;
        } else {
            self.queue_captain_steer_for_review(session_id, content)
                .await?;
        }

        Ok(mode)
    }

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
                        block: ContentBlock::Text { text: summary },
                    },
                );
            }

            transition_task(active, TaskStatus::Accepted).map_err(|error| error.to_string())?;
            archive_terminal_task(active);
        }

        self.persist_session(session_id).await
    }

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

    async fn captain_tool_steer(
        &self,
        session_id: &SessionId,
        message: String,
    ) -> Result<String, String> {
        let mode = self
            .captain_delegate_to_mate(session_id, message.clone())
            .await?;
        self.append_captain_tool_call(
            session_id,
            "ship_steer",
            json!({ "message": message }).to_string(),
            vec![ToolCallContent::Text {
                text: if mode == AutonomyMode::Autonomous {
                    "Forwarded steer to the mate.".to_owned()
                } else {
                    "Queued steer for human review.".to_owned()
                },
            }],
        )
        .await?;

        Ok(if mode == AutonomyMode::Autonomous {
            "Steer sent to the mate.".to_owned()
        } else {
            "Steer queued for human review.".to_owned()
        })
    }

    async fn captain_tool_accept(
        &self,
        session_id: &SessionId,
        summary: Option<String>,
    ) -> Result<String, String> {
        self.accept_task(session_id, summary.clone()).await?;
        self.append_captain_tool_call(
            session_id,
            "ship_accept",
            json!({ "summary": summary }).to_string(),
            vec![ToolCallContent::Text {
                text: "Accepted the active task.".to_owned(),
            }],
        )
        .await?;
        Ok("Accepted the active task.".to_owned())
    }

    async fn captain_tool_reject(
        &self,
        session_id: &SessionId,
        reason: String,
        message: Option<String>,
    ) -> Result<String, String> {
        let detail = message
            .clone()
            .map(|message| format!("{reason}: {message}"))
            .unwrap_or_else(|| reason.clone());
        self.cancel_task(session_id, Some(detail.clone())).await?;
        self.append_captain_tool_call(
            session_id,
            "ship_reject",
            json!({ "reason": reason, "message": message }).to_string(),
            vec![ToolCallContent::Text {
                text: detail.clone(),
            }],
        )
        .await?;
        Ok("Rejected the active task.".to_owned())
    }

    async fn start_session_runtime(&self, session_id: SessionId) {
        let stage = SessionStartupStage::ResolvingMcp;
        let _ = self
            .set_startup_state(
                &session_id,
                SessionStartupState::Running {
                    stage,
                    message: Self::startup_message(stage).to_owned(),
                },
            )
            .await;

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

        let repo_root = match self.resolve_project_root(&project).await {
            Ok(value) => value,
            Err(error) => {
                self.fail_startup(&session_id, stage, error).await;
                return;
            }
        };

        let stage = SessionStartupStage::CreatingWorktree;
        let _ = self
            .set_startup_state(
                &session_id,
                SessionStartupState::Running {
                    stage,
                    message: Self::startup_message(stage).to_owned(),
                },
            )
            .await;
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
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(session) = sessions.get_mut(&session_id) {
                session.worktree_path = Some(worktree_path.clone());
            }
        }
        let _ = self.persist_session(&session_id).await;

        let captain_ship_mcp = match self
            .install_captain_mcp_server(&session_id, &worktree_path)
            .await
        {
            Ok(config) => config,
            Err(error) => {
                self.fail_startup(&session_id, SessionStartupStage::StartingCaptain, error)
                    .await;
                return;
            }
        };

        let stage = SessionStartupStage::StartingCaptain;
        let _ = self
            .set_startup_state(
                &session_id,
                SessionStartupState::Running {
                    stage,
                    message: Self::startup_message(stage).to_owned(),
                },
            )
            .await;
        let captain_handle = match self
            .agent_driver
            .spawn(
                {
                    let sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    sessions
                        .get(&session_id)
                        .expect("session exists")
                        .config
                        .captain_kind
                },
                Role::Captain,
                &AgentSessionConfig {
                    worktree_path: worktree_path.clone(),
                    mcp_servers: {
                        let mut servers = resolved_mcp_servers.clone();
                        servers.push(captain_ship_mcp);
                        servers
                    },
                },
            )
            .await
        {
            Ok(handle) => handle,
            Err(error) => {
                self.clear_captain_mcp_server(&session_id);
                self.fail_startup(&session_id, stage, error.message).await;
                return;
            }
        };
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(session) = sessions.get_mut(&session_id) {
                session.captain_handle = Some(captain_handle);
            }
        }
        let _ = self.persist_session(&session_id).await;

        let stage = SessionStartupStage::StartingMate;
        let _ = self
            .set_startup_state(
                &session_id,
                SessionStartupState::Running {
                    stage,
                    message: Self::startup_message(stage).to_owned(),
                },
            )
            .await;
        let mate_handle = match self
            .agent_driver
            .spawn(
                {
                    let sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    sessions
                        .get(&session_id)
                        .expect("session exists")
                        .config
                        .mate_kind
                },
                Role::Mate,
                &AgentSessionConfig {
                    worktree_path: worktree_path.clone(),
                    mcp_servers: resolved_mcp_servers.clone(),
                },
            )
            .await
        {
            Ok(handle) => handle,
            Err(error) => {
                self.fail_startup(&session_id, stage, error.message).await;
                return;
            }
        };
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(session) = sessions.get_mut(&session_id) {
                session.mate_handle = Some(mate_handle);
            }
        }
        let _ = self.persist_session(&session_id).await;

        let stage = SessionStartupStage::GreetingCaptain;
        let _ = self
            .set_startup_state(
                &session_id,
                SessionStartupState::Running {
                    stage,
                    message: Self::startup_message(stage).to_owned(),
                },
            )
            .await;
        if let Err(error) = self
            .prompt_agent(&session_id, Role::Captain, Self::captain_bootstrap_prompt())
            .await
        {
            self.fail_startup(&session_id, stage, error).await;
            return;
        }

        let _ = self
            .set_startup_state(&session_id, SessionStartupState::Ready)
            .await;
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
            },
            mate: AgentSnapshot {
                role: Role::Mate,
                kind: req.mate_kind,
                state: AgentState::Idle,
                context_remaining_percent: None,
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

        let this = self.clone();
        let startup_session_id = session_id.clone();
        tokio::spawn(async move {
            this.start_session_runtime(startup_session_id).await;
        });

        CreateSessionResponse::Created { session_id }
    }

    async fn assign(&self, session: SessionId, description: String) -> AssignTaskResponse {
        match self.start_task(&session, description).await {
            Ok(task_id) => {
                let this = self.clone();
                let session_for_prompt = session.clone();
                let description_for_prompt = {
                    let sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    sessions
                        .get(&session)
                        .and_then(|active| active.current_task.as_ref())
                        .map(|task| task.record.description.clone())
                        .unwrap_or_default()
                };
                tokio::spawn(async move {
                    if let Err(error) = this
                        .prompt_agent(
                            &session_for_prompt,
                            Role::Captain,
                            Self::captain_assignment_prompt(&description_for_prompt),
                        )
                        .await
                    {
                        Self::log_error("prompt_captain_assign", &error);
                    }
                });
                AssignTaskResponse::Assigned { task_id }
            }
            Err(error) => {
                Self::log_error("assign", &error);
                AssignTaskResponse::Failed { message: error }
            }
        }
    }

    async fn steer(&self, session: SessionId, content: String) {
        if let Err(error) = self.dispatch_steer_to_mate(&session, content).await {
            Self::log_error("steer", &error);
        }
    }

    // r[acp.prompt]
    async fn prompt_captain(&self, session: SessionId, content: String) {
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
        self.clear_captain_mcp_server(&req.id);
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
    async fn captain_tool_steer_queues_review_in_human_mode() {
        let (dir, ship, session_id) =
            create_session_for_workflow_test("captain-tool-steer-review").await;

        let result = ship
            .captain_tool_steer(&session_id, "Ask the mate to add coverage".to_owned())
            .await
            .expect("captain tool should succeed");

        assert_eq!(result, "Steer queued for human review.");

        let detail = Ship::get_session(&ship, session_id.clone()).await;
        assert_eq!(
            detail
                .current_task
                .as_ref()
                .expect("task should exist")
                .status,
            TaskStatus::SteerPending
        );
        assert_eq!(
            detail.pending_steer.as_deref(),
            Some("Ask the mate to add coverage")
        );

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
