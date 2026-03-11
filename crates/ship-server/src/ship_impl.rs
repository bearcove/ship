use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
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
    JsonSessionStore, PendingEdit, ProjectRegistry, SessionStore, WorktreeOps, apply_event,
    archive_terminal_task, current_task_status, rebuild_materialized_from_event_log,
    resolve_mcp_servers, set_agent_state, transition_task,
};
use ship_service::{CaptainMcp, CaptainMcpDispatcher, MateMcp, MateMcpDispatcher, Ship};
use ship_types::{
    AgentDiscovery, AgentKind, AgentSnapshot, AgentState, AutonomyMode, BlockId,
    CloseSessionRequest, CloseSessionResponse, ContentBlock, CreateSessionRequest,
    CreateSessionResponse, CurrentTask, HumanReviewRequest, McpDiffContent, McpServerConfig,
    McpStdioServerConfig, McpToolCallResponse, PersistedSession, PlanStep, PlanStepPriority,
    PlanStepStatus, ProjectInfo, ProjectName, PromptContentPart, Role, SessionConfig,
    SessionDetail, SessionEvent, SessionEventEnvelope, SessionId, SessionStartupStage,
    SessionStartupState, SessionSummary, SetAgentEffortResponse, SetAgentModelResponse,
    SubscribeMessage, TaskId, TaskRecord, TaskStatus, ToolCallKind, ToolTarget,
};
use tokio::process::Command as TokioCommand;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

fn parts_to_log_text(parts: &[PromptContentPart]) -> String {
    let mut out = String::new();
    for part in parts {
        match part {
            PromptContentPart::Text { text } => {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
            PromptContentPart::Image { mime_type, .. } => {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&format!("[image: {mime_type}]"));
            }
        }
    }
    out
}

const FILE_MENTION_LINE_LIMIT: usize = 200;
const MAX_WORKTREE_FILES: usize = 5000;

const PLAN_REQUIRED_MESSAGE: &str = "\
Before you can write files, run commands, or make edits, you need to lay out \
a plan using set_plan. Read the relevant code, understand the problem, then \
call set_plan with the steps you intend to take. Once your plan is in place, \
you can start working.";

const BLOCKED_COMMAND_MESSAGE: &str = "\
That command has been blocked because it could damage the worktree in ways \
that are hard to undo. Stop what you're doing and use mate_ask_captain to \
explain what you were trying to accomplish — the captain can help you find \
a safe approach.";

const RUN_COMMAND_GUARDRAIL_TEMPLATE: &str = "\
The command `{command}` has been blocked because it could affect the worktree \
in ways that are hard to undo. Use mate_ask_captain to explain what you need, \
and the captain will help you find the right approach.";
const DEFAULT_READ_FILE_LIMIT: usize = 2000;
const MAX_READ_FILE_LINE_LENGTH: usize = 2000;
const BINARY_DETECTION_BYTES: usize = 8 * 1024;
const MAX_TOOL_OUTPUT_LINES: usize = 1000;
const MAX_TOOL_OUTPUT_BYTES: usize = 50 * 1024;
const MATE_TOOL_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const RUN_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

struct AutoCommitResult {
    commit_hash: String,
    diff_stat: String,
}

struct ReplacementOccurrence {
    old_start: usize,
    old_end: usize,
    new_start: usize,
    new_end: usize,
}

struct PreparedEdit {
    pending: PendingEdit,
    diff: String,
}

enum RustfmtOutcome {
    Success,
    NotFound,
    Failure(String),
}

#[cfg(test)]
static TEST_RUSTFMT_PROGRAM: Mutex<Option<std::ffi::OsString>> = Mutex::new(None);
#[cfg(test)]
static TEST_RG_PROGRAM: Mutex<Option<std::ffi::OsString>> = Mutex::new(None);
#[cfg(test)]
static TEST_FD_PROGRAM: Mutex<Option<std::ffi::OsString>> = Mutex::new(None);

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
    /// Sender to unblock a mid-task `set_plan` call when the captain approves or rejects.
    /// Carries the old plan so it can be restored on rejection.
    plan_change_reply: Option<(
        Vec<PlanStep>,
        tokio::sync::oneshot::Sender<Result<(), String>>,
    )>,
}

impl PendingMcpOps {
    fn new() -> Self {
        Self {
            human_reply: None,
            captain_reply: None,
            mate_review: None,
            plan_change_reply: None,
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
    user_avatar_url: Arc<Mutex<Option<String>>>,
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
            user_avatar_url: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn fetch_github_user_avatar(&self) {
        let output = TokioCommand::new("gh")
            .args(["api", "/user"])
            .output()
            .await;
        let Ok(output) = output else {
            return;
        };
        if !output.status.success() {
            return;
        }
        let Ok(text) = std::str::from_utf8(&output.stdout) else {
            return;
        };
        // Parse avatar_url from JSON without pulling in a full JSON library:
        // look for "avatar_url":"<url>"
        let avatar_url = text
            .split("\"avatar_url\":")
            .nth(1)
            .and_then(|s| s.trim().strip_prefix('"'))
            .and_then(|s| s.split('"').next())
            .map(ToOwned::to_owned);
        if let Some(url) = avatar_url {
            tracing::info!(%url, "fetched github user avatar");
            *self
                .user_avatar_url
                .lock()
                .expect("user_avatar_url mutex poisoned") = Some(url);
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
                created_at: persisted.created_at,
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
                pending_edits: HashMap::new(),
                pending_steer: None,
                pending_human_review: None,
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
            effort_config_id: None,
            effort_value_id: None,
            available_effort_values: Vec::new(),
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
            current_task_title: session
                .current_task
                .as_ref()
                .map(|task| task.record.title.clone()),
            current_task_description: session
                .current_task
                .as_ref()
                .map(|task| task.record.description.clone()),
            task_status: session.current_task.as_ref().map(|task| task.record.status),
            autonomy_mode: session.config.autonomy_mode,
            created_at: session.created_at.clone(),
        }
    }

    fn to_session_detail(
        session: &ActiveSession,
        user_avatar_url: Option<String>,
    ) -> SessionDetail {
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
            pending_human_review: session.pending_human_review.clone(),
            created_at: session.created_at.clone(),
            user_avatar_url,
        }
    }

    fn fallback_session_detail(id: SessionId, user_avatar_url: Option<String>) -> SessionDetail {
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
            pending_human_review: None,
            created_at: String::new(),
            user_avatar_url,
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
            SessionEvent::AgentEffortChanged { .. } => "AgentEffortChanged",
            SessionEvent::MateGuidanceQueued { .. } => "MateGuidanceQueued",
            SessionEvent::HumanReviewRequested { .. } => "HumanReviewRequested",
            SessionEvent::HumanReviewCleared => "HumanReviewCleared",
        }
    }

    fn event_role(event: &SessionEvent) -> Option<Role> {
        match event {
            SessionEvent::BlockAppend { role, .. }
            | SessionEvent::BlockPatch { role, .. }
            | SessionEvent::AgentStateChanged { role, .. }
            | SessionEvent::ContextUpdated { role, .. }
            | SessionEvent::AgentModelChanged { role, .. }
            | SessionEvent::AgentEffortChanged { role, .. } => Some(*role),
            SessionEvent::SessionStartupChanged { .. }
            | SessionEvent::TaskStatusChanged { .. }
            | SessionEvent::TaskStarted { .. }
            | SessionEvent::MateGuidanceQueued { .. }
            | SessionEvent::HumanReviewRequested { .. }
            | SessionEvent::HumanReviewCleared => None,
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
            | SessionEvent::AgentModelChanged { .. }
            | SessionEvent::AgentEffortChanged { .. }
            | SessionEvent::MateGuidanceQueued { .. }
            | SessionEvent::HumanReviewRequested { .. }
            | SessionEvent::HumanReviewCleared => None,
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
            | SessionEvent::AgentModelChanged { .. }
            | SessionEvent::AgentEffortChanged { .. }
            | SessionEvent::MateGuidanceQueued { .. }
            | SessionEvent::HumanReviewRequested { .. }
            | SessionEvent::HumanReviewCleared => None,
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
        "\
You are the captain — a senior engineer who plans, reviews, and steers, but \
never writes code directly.

A human will describe what they want built or fixed. Your job is to understand \
the request, ask clarifying questions if anything is unclear, and then delegate \
the actual implementation to the mate by calling captain_assign with a clear \
task description.

Here's how a typical cycle works:

1. The human describes a goal.
2. You discuss it with the human until the scope is clear.
3. You call captain_assign to hand the work to the mate.
4. The mate creates a plan, implements it, and calls mate_submit when done.
5. You review the mate's work and either accept it (captain_accept), give \
   feedback (captain_steer), or cancel it (captain_cancel).

You can also steer the mate mid-flight with captain_steer if you see it going \
off track, or notify the human with captain_notify_human if you need their input.

Your available tools are your Ship MCP tools: captain_assign, captain_steer, \
captain_accept, captain_cancel, captain_notify_human, read_file, search_files, \
list_files, and web_search. Built-in tools (Bash, Read, Write, Edit) are \
disabled in this environment. If you try one and it fails or is rejected, do \
not stop — use your MCP tools instead and continue.

Right now, a new session has just started and there is no active task. Greet \
the human briefly and wait for them to describe what they'd like to work on."
            .to_owned()
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
        parts: &[PromptContentPart],
    ) -> Result<(), String> {
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let log_text = parts_to_log_text(parts);
            apply_event(
                session,
                SessionEvent::BlockAppend {
                    block_id: BlockId::new(),
                    role,
                    block: ContentBlock::Text {
                        text: log_text,
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
        parts: Vec<PromptContentPart>,
    ) -> Result<(), String> {
        let already_working = {
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

            let log_text = parts_to_log_text(&parts);
            apply_event(
                active,
                SessionEvent::BlockAppend {
                    block_id: BlockId::new(),
                    role: Role::Mate,
                    block: ContentBlock::Text {
                        text: log_text.clone(),
                        source: ship_types::TextSource::Human,
                    },
                },
            );

            let already_working = status == TaskStatus::Working;
            if already_working {
                // Inject as pending guidance for the running loop to pick up
                // at the end of the current turn. Prefix matches what
                // prompt_mate_from_steer prepends to initial parts.
                let guidance = format!("Captain steer:\n{log_text}");
                if let Some(task) = active.current_task.as_mut() {
                    task.pending_mate_guidance = Some(guidance);
                }
            } else {
                transition_task(active, TaskStatus::Working).map_err(|error| error.to_string())?;
            }
            active.pending_steer = None;

            already_working
        };

        self.persist_session(session_id).await?;

        if already_working {
            // Guidance is queued; the running loop will deliver it after the
            // current turn. No cancel needed — cancelling risks a race where
            // the cancel arrives after the loop has already started a new
            // turn from the guidance, archiving the task incorrectly.
            return Ok(());
        }

        let this = self.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            this.prompt_mate_from_steer(session_id, parts).await;
        });

        Ok(())
    }

    // r[task.accept]
    // r[task.accept]
    async fn accept_task(
        &self,
        session_id: &SessionId,
        summary: Option<String>,
    ) -> Result<(), String> {
        let (worktree_path, base_branch, branch_name) = {
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

            let worktree_path = active
                .worktree_path
                .clone()
                .ok_or_else(|| "session worktree not ready".to_owned())?;
            (
                worktree_path,
                active.config.base_branch.clone(),
                active.config.branch_name.clone(),
            )
        };

        self.persist_session(session_id).await?;

        // Rebase the session branch onto the base branch, then fast-forward
        // merge so the work lands on main with no merge commits.
        let repo_root = Self::repo_root_for_worktree(&worktree_path)
            .map_err(|error| error.to_string())?
            .to_path_buf();

        self.worktree_ops
            .rebase_onto(&worktree_path, &base_branch)
            .await
            .map_err(|error| format!("rebase failed: {}", error.message))?;

        self.worktree_ops
            .merge_ff_only(&repo_root, &branch_name)
            .await
            .map_err(|error| format!("fast-forward merge failed: {}", error.message))?;

        Ok(())
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

        let mate_spawn = self
            .agent_driver
            .spawn(mate_kind, Role::Mate, &mate_config)
            .await
            .map_err(|error| error.message)?;

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(session) = sessions.get_mut(session_id) {
                session.mate_handle = Some(mate_spawn.handle);
                session.mate.model_id = mate_spawn.model_id;
                session.mate.available_models = mate_spawn.available_models;
                session.mate.effort_config_id = mate_spawn.effort_config_id;
                session.mate.effort_value_id = mate_spawn.effort_value_id;
                session.mate.available_effort_values = mate_spawn.available_effort_values;
            }
        }

        Ok(())
    }

    // r[mate.system-prompt]
    fn mate_task_preamble(description: &str) -> String {
        format!(
            "<system-notification>\
You are the mate — an implementation-focused engineer. The captain has just \
assigned you a task. Your job is to write code, run tests, and get things done.

Here's how you work:

1. Read the relevant files and understand the problem.
2. Call set_plan with a list of steps you intend to take. You cannot write \
   files, run commands, or make edits until you've done this.
3. Work ONE step at a time: complete the step fully, then immediately call \
   plan_step_complete before starting the next step. Never work ahead. \
   Steps that produce file changes are committed individually — one step, \
   one commit. Steps with no file changes (research, investigation) are \
   marked complete without a commit.
4. When you're done, call mate_submit with a summary of all your changes.
5. After calling mate_submit, do not send any further messages. The tool \
   call is the final action — the submission itself carries the summary.

If you get stuck or need a decision, call mate_ask_captain — it will block \
until the captain responds, so use it when you genuinely need direction.

You can also send non-blocking progress updates with mate_send_update if you \
want to keep the captain informed without waiting for a reply.

All your file operations go through Ship's MCP tools: run_command (shell \
commands), read_file, write_file, edit_prepare/edit_confirm, search_files, \
list_files. Built-in tools (Bash, Read, Write, Edit) are disabled — if you \
try one and see an error or a rejection, do not stop. Just use the MCP \
equivalent and continue your task.

Here is your task:

{description}\
</system-notification>"
        )
    }

    // r[task.assign]
    // r[captain.tool.assign]
    async fn captain_tool_assign(
        &self,
        session_id: &SessionId,
        title: String,
        description: String,
        keep: bool,
    ) -> Result<String, String> {
        let task_id = self
            .start_task(session_id, title.clone(), description.clone())
            .await?;

        if !keep {
            self.restart_mate(session_id).await?;
        }

        let mate_prompt = Self::mate_task_preamble(&description);

        let this = self.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this
                .dispatch_steer_to_mate(
                    &session_id,
                    vec![PromptContentPart::Text { text: mate_prompt }],
                )
                .await
            {
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
        // If the mate is blocked on a mid-task plan change, reject it and redirect.
        let pending_plan_change = self
            .pending_mcp_ops
            .lock()
            .expect("pending_mcp_ops mutex poisoned")
            .get_mut(session_id)
            .and_then(|ops| ops.plan_change_reply.take());

        if let Some((old_plan, tx)) = pending_plan_change {
            {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                if let Some(session) = sessions.get_mut(session_id) {
                    if let Some(task) = session.current_task.as_mut() {
                        task.mate_plan = Some(old_plan);
                    }
                }
            }
            self.persist_session(session_id).await?;
            let _ = tx.send(Err(message));
            return Ok("Plan change rejected; mate redirected.".to_owned());
        }

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
                self.dispatch_steer_to_mate(
                    session_id,
                    vec![PromptContentPart::Text {
                        text: message.clone(),
                    }],
                )
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
        // If the mate is blocked on a mid-task plan change, approve it.
        let pending_plan_change = self
            .pending_mcp_ops
            .lock()
            .expect("pending_mcp_ops mutex poisoned")
            .get_mut(session_id)
            .and_then(|ops| ops.plan_change_reply.take());

        if let Some((_, tx)) = pending_plan_change {
            let _ = tx.send(Ok(()));
            return Ok("Plan change accepted; mate unblocked.".to_owned());
        }

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

    // r[captain.tool.notify-human]
    async fn captain_tool_notify_human(
        &self,
        session_id: &SessionId,
        message: String,
    ) -> Result<String, String> {
        // Compute the diff and get the worktree path while holding the lock briefly.
        let (worktree_path, base_branch) = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let worktree_path = session
                .worktree_path
                .clone()
                .ok_or_else(|| "session worktree not ready".to_owned())?;
            (worktree_path, session.config.base_branch.clone())
        };

        // git diff base_branch...HEAD — shows everything the session branch adds.
        let diff = {
            let output = TokioCommand::new("git")
                .arg("-C")
                .arg(&worktree_path)
                .arg("diff")
                .arg(format!("{base_branch}...HEAD"))
                .output()
                .await
                .map_err(|e| format!("failed to compute diff: {e}"))?;
            String::from_utf8_lossy(&output.stdout).into_owned()
        };

        let review = HumanReviewRequest {
            message: message.clone(),
            diff,
            worktree_path: worktree_path.display().to_string(),
        };

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            session.pending_human_review = Some(review.clone());
            apply_event(
                session,
                SessionEvent::HumanReviewRequested {
                    message: review.message.clone(),
                    diff: review.diff.clone(),
                    worktree_path: review.worktree_path.clone(),
                },
            );
        }
        self.persist_session(session_id)
            .await
            .map_err(|e| format!("persist failed: {e}"))?;

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

        let reply = match rx.await {
            Ok(reply) => reply,
            Err(_) => return Err("human reply channel closed".to_owned()),
        };

        // Clear the pending review state.
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(session) = sessions.get_mut(session_id) {
                session.pending_human_review = None;
                apply_event(session, SessionEvent::HumanReviewCleared);
            }
        }
        let _ = self.persist_session(session_id).await;

        Ok(reply)
    }

    // r[captain.tool.read-only]
    async fn captain_tool_read_file(
        &self,
        session_id: &SessionId,
        path: String,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> Result<String, String> {
        self.mate_tool_read_file(session_id, path, offset, limit)
            .await
    }

    // r[captain.tool.read-only]
    async fn captain_tool_search_files(
        &self,
        session_id: &SessionId,
        args: String,
    ) -> Result<String, String> {
        self.mate_tool_search_files(session_id, args).await
    }

    // r[captain.tool.read-only]
    async fn captain_tool_list_files(
        &self,
        session_id: &SessionId,
        args: String,
    ) -> Result<String, String> {
        self.mate_tool_list_files(session_id, args).await
    }

    async fn mate_tool_networked_cargo(
        &self,
        session_id: &SessionId,
        subcommand: &str,
        extra_args: Option<String>,
    ) -> Result<String, String> {
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            Self::current_task_worktree_path(session)?.to_path_buf()
        };
        let args = match extra_args {
            Some(a) if !a.trim().is_empty() => format!("{subcommand} {a}"),
            _ => subcommand.to_owned(),
        };
        let shell_command = format!("exec 2>&1; cargo {args}");
        let child = Self::networked_sandboxed_sh(&worktree_path, &shell_command)?;
        let output = match tokio::time::timeout(RUN_COMMAND_TIMEOUT, child.wait_with_output()).await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => return Err(format!("Command failed: {error}")),
            Err(_) => return Err("Command timed out after 120 seconds.".to_owned()),
        };
        let combined = String::from_utf8_lossy(&output.stdout);
        let truncated = Self::truncate_run_command_output(&combined);
        let exit_code = output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |c| c.to_string());
        if truncated.is_empty() {
            Ok(format!("exit code: {exit_code}"))
        } else {
            Ok(format!("{truncated}\nexit code: {exit_code}"))
        }
    }

    // r[mate.tool.cargo-check]
    async fn mate_tool_cargo_check(
        &self,
        session_id: &SessionId,
        args: Option<String>,
    ) -> Result<String, String> {
        self.mate_tool_networked_cargo(session_id, "check", args)
            .await
    }

    // r[mate.tool.cargo-clippy]
    async fn mate_tool_cargo_clippy(
        &self,
        session_id: &SessionId,
        args: Option<String>,
    ) -> Result<String, String> {
        self.mate_tool_networked_cargo(session_id, "clippy", args)
            .await
    }

    // r[mate.tool.cargo-test]
    async fn mate_tool_cargo_test(
        &self,
        session_id: &SessionId,
        args: Option<String>,
    ) -> Result<String, String> {
        self.mate_tool_networked_cargo(session_id, "nextest run", args)
            .await
    }

    // r[mate.tool.pnpm-install]
    async fn mate_tool_pnpm_install(
        &self,
        session_id: &SessionId,
        args: Option<String>,
    ) -> Result<String, String> {
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            Self::current_task_worktree_path(session)?.to_path_buf()
        };
        let cmd_str = match args {
            Some(a) if !a.trim().is_empty() => format!("pnpm install {a}"),
            _ => "pnpm install".to_owned(),
        };
        let shell_command = format!("exec 2>&1; {cmd_str}");
        let child = Self::networked_sandboxed_sh(&worktree_path, &shell_command)?;
        let output = match tokio::time::timeout(RUN_COMMAND_TIMEOUT, child.wait_with_output()).await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => return Err(format!("Command failed: {error}")),
            Err(_) => return Err("Command timed out after 120 seconds.".to_owned()),
        };
        let combined = String::from_utf8_lossy(&output.stdout);
        let truncated = Self::truncate_run_command_output(&combined);
        let exit_code = output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |c| c.to_string());
        if truncated.is_empty() {
            Ok(format!("exit code: {exit_code}"))
        } else {
            Ok(format!("{truncated}\nexit code: {exit_code}"))
        }
    }

    fn format_plan_status(steps: &[PlanStep]) -> String {
        steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let marker = match step.status {
                    PlanStepStatus::Completed => "[x]",
                    _ => "[ ]",
                };
                format!("{marker} Step {}: {}", index + 1, step.description)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn build_plan_steps(steps: Vec<String>) -> Vec<PlanStep> {
        steps
            .into_iter()
            .map(|description| PlanStep {
                description,
                priority: PlanStepPriority::Medium,
                status: PlanStepStatus::Pending,
            })
            .collect()
    }

    fn current_task_worktree_path(session: &ActiveSession) -> Result<&std::path::Path, String> {
        session
            .worktree_path
            .as_deref()
            .ok_or_else(|| "session worktree is not ready".to_owned())
    }

    fn validate_worktree_path(path: &str) -> Result<&Path, String> {
        let candidate = Path::new(path);
        if candidate.is_absolute() {
            return Err("Absolute paths are not allowed.".to_owned());
        }
        if candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err("Path resolves outside the worktree.".to_owned());
        }
        Ok(candidate)
    }

    fn resolve_worktree_file_path(
        canonical_worktree: &Path,
        relative_path: &Path,
    ) -> Result<PathBuf, String> {
        let parent = relative_path
            .parent()
            .map(|parent| canonical_worktree.join(parent))
            .unwrap_or_else(|| canonical_worktree.to_path_buf());
        fs::create_dir_all(&parent)
            .map_err(|error| format!("Failed to create parent directory: {error}"))?;
        let canonical_parent = fs::canonicalize(&parent).map_err(|error| {
            format!(
                "Failed to resolve parent directory path {}: {error}",
                parent.display()
            )
        })?;
        if !canonical_parent.starts_with(canonical_worktree) {
            return Err("Path resolves outside the worktree.".to_owned());
        }
        let Some(file_name) = relative_path.file_name() else {
            return Err("Path must point to a file.".to_owned());
        };
        Ok(canonical_parent.join(file_name))
    }

    fn line_count(content: &str) -> usize {
        if content.is_empty() {
            0
        } else {
            content.lines().count()
        }
    }

    fn rustfmt_program() -> std::ffi::OsString {
        #[cfg(test)]
        if let Some(program) = TEST_RUSTFMT_PROGRAM
            .lock()
            .expect("test rustfmt program mutex poisoned")
            .clone()
        {
            return program;
        }

        std::ffi::OsString::from("rustfmt")
    }

    fn rg_program() -> std::ffi::OsString {
        #[cfg(test)]
        if let Some(program) = TEST_RG_PROGRAM
            .lock()
            .expect("test rg program mutex poisoned")
            .clone()
        {
            return program;
        }

        std::ffi::OsString::from("rg")
    }

    fn fd_program() -> std::ffi::OsString {
        #[cfg(test)]
        if let Some(program) = TEST_FD_PROGRAM
            .lock()
            .expect("test fd program mutex poisoned")
            .clone()
        {
            return program;
        }

        std::ffi::OsString::from("fd")
    }

    fn run_rustfmt(program: &std::ffi::OsString, path: &Path) -> Result<RustfmtOutcome, String> {
        let mut command = Command::new(program);
        let output = match command.arg("--edition").arg("2024").arg(path).output() {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RustfmtOutcome::NotFound);
            }
            Err(error) => return Err(format!("failed to run rustfmt: {error}")),
        };

        if output.status.success() {
            return Ok(RustfmtOutcome::Success);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "rustfmt failed".to_owned()
        };
        Ok(RustfmtOutcome::Failure(details))
    }

    fn truncate_tool_output(output: &str) -> String {
        let output = output.trim_end_matches('\n');
        if output.is_empty() {
            return String::new();
        }

        if output.len() <= MAX_TOOL_OUTPUT_BYTES && output.lines().count() <= MAX_TOOL_OUTPUT_LINES
        {
            return output.to_owned();
        }

        let lines = output.lines().collect::<Vec<_>>();
        let total_lines = lines.len();
        let mut rendered = String::new();

        for line in lines.iter().take(MAX_TOOL_OUTPUT_LINES) {
            let line_len = line.len();
            let projected_len = if rendered.is_empty() {
                line_len
            } else {
                rendered.len() + 1 + line_len
            };
            if projected_len > MAX_TOOL_OUTPUT_BYTES {
                break;
            }

            if !rendered.is_empty() {
                rendered.push('\n');
            }
            rendered.push_str(line);
        }

        if !rendered.is_empty() {
            rendered.push('\n');
        }
        rendered.push_str(&format!(
            "(output truncated - {total_lines} lines total. Narrow your search.)"
        ));
        rendered
    }

    fn truncate_run_command_output(output: &str) -> String {
        let output = output.trim_end_matches('\n');
        let lines = output.lines().collect::<Vec<_>>();
        if lines.len() <= MAX_TOOL_OUTPUT_LINES {
            return output.to_owned();
        }

        let mut rendered = lines
            .iter()
            .take(MAX_TOOL_OUTPUT_LINES)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        rendered.push_str(&format!(
            "(output truncated - {} lines total, showing first {} lines.)",
            lines.len(),
            MAX_TOOL_OUTPUT_LINES
        ));
        rendered
    }

    // r[mate.tool.sandbox]
    fn sandboxed_sh(cwd: &Path, shell_command: &str) -> Result<tokio::process::Child, String> {
        Self::spawn_sh(cwd, shell_command, false)
    }

    // r[mate.tool.networked-sandbox]
    fn networked_sandboxed_sh(
        cwd: &Path,
        shell_command: &str,
    ) -> Result<tokio::process::Child, String> {
        Self::spawn_sh(cwd, shell_command, true)
    }

    fn spawn_sh(
        cwd: &Path,
        shell_command: &str,
        allow_network: bool,
    ) -> Result<tokio::process::Child, String> {
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/nonexistent".to_owned());
            let system_tmpdir = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned());
            let worktree = cwd.to_string_lossy();
            let extra_rules = if allow_network {
                format!(
                    "\n(allow file-write* (subpath \"{home}/.cargo\"))\
                    \n(allow file-write* (subpath \"{home}/.rustup\"))"
                )
            } else {
                "\n(deny network*)".to_owned()
            };
            let profile = format!(
                "(version 1)\
                \n(allow default)\
                \n(deny file-write* (subpath \"/\"))\
                \n(allow file-write* (subpath \"{worktree}\"))\
                \n(allow file-write* (subpath \"/private/tmp\"))\
                \n(allow file-write* (subpath \"/tmp\"))\
                \n(allow file-write* (subpath \"{system_tmpdir}\"))\
                \n(allow file-write* (literal \"/dev/null\"))\
                {extra_rules}"
            );
            let mut cmd = TokioCommand::new("/usr/bin/sandbox-exec");
            cmd.arg("-p")
                .arg(profile)
                .arg("/bin/sh")
                .arg("-c")
                .arg(shell_command)
                .current_dir(cwd)
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped());
            cmd.spawn()
                .map_err(|error| format!("Failed to start sandboxed command: {error}"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = allow_network;
            let mut cmd = TokioCommand::new("/bin/sh");
            cmd.arg("-c")
                .arg(shell_command)
                .current_dir(cwd)
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped());
            cmd.spawn()
                .map_err(|error| format!("Failed to start command: {error}"))
        }
    }

    async fn run_worktree_shell_command(
        worktree_path: PathBuf,
        program: std::ffi::OsString,
        args: String,
        missing_program_message: &'static str,
        no_matches_message: Option<&'static str>,
    ) -> Result<String, String> {
        let command_text = if args.trim().is_empty() {
            program.to_string_lossy().into_owned()
        } else {
            format!("{} {}", program.to_string_lossy(), args)
        };

        let augmented_path = {
            let base = std::env::var("PATH").unwrap_or_default();
            let extra = [
                "/Users/amos/.cargo/bin",
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
            ];
            let mut parts: Vec<&str> = base.split(':').collect();
            for dir in extra {
                if !parts.contains(&dir) {
                    parts.push(dir);
                }
            }
            parts.join(":")
        };

        let tmp_dir = worktree_path.join(".tmp");
        let _ = tokio::fs::create_dir_all(&tmp_dir).await;
        let _ = tokio::fs::write(tmp_dir.join(".gitignore"), "*\n").await;

        let mut command = TokioCommand::new("/bin/sh");
        command
            .arg("-c")
            .arg(&command_text)
            .current_dir(&worktree_path)
            .env("PATH", &augmented_path)
            .env("TMPDIR", &tmp_dir)
            .env("TMP", &tmp_dir)
            .env("TEMP", &tmp_dir)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = command
            .spawn()
            .map_err(|error| format!("failed to start command: {error}"))?;

        let output =
            match tokio::time::timeout(MATE_TOOL_COMMAND_TIMEOUT, child.wait_with_output()).await {
                Ok(Ok(output)) => output,
                Ok(Err(error)) => return Err(format!("command execution failed: {error}")),
                Err(_) => return Err("command timed out after 30 seconds".to_owned()),
            };

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Ok(Self::truncate_tool_output(&stdout));
        }

        if output.status.code() == Some(1) {
            if let Some(no_matches_message) = no_matches_message {
                return Ok(no_matches_message.to_owned());
            }
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if output.status.code() == Some(127)
            || stderr.contains("not found")
            || stderr.contains("command not found")
        {
            return Err(missing_program_message.to_owned());
        }

        if !stderr.is_empty() {
            return Err(stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !stdout.is_empty() {
            return Err(stdout);
        }

        Err("command failed".to_owned())
    }

    fn resolve_worktree_directory(
        canonical_worktree: &Path,
        relative_path: &Path,
    ) -> Result<PathBuf, String> {
        let candidate_path = canonical_worktree.join(relative_path);
        let metadata = fs::metadata(&candidate_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                format!("Directory not found: {}", relative_path.display())
            } else {
                format!("Failed to access directory: {error}")
            }
        })?;
        if !metadata.is_dir() {
            return Err("cwd must point to a directory.".to_owned());
        }

        let canonical_dir = fs::canonicalize(&candidate_path).map_err(|error| {
            format!(
                "Failed to resolve directory path {}: {error}",
                candidate_path.display()
            )
        })?;
        if !canonical_dir.starts_with(canonical_worktree) {
            return Err("Path resolves outside the worktree.".to_owned());
        }

        Ok(canonical_dir)
    }

    fn restore_written_file(target: &Path, backup: Option<&Path>) -> Result<(), String> {
        if target.exists() {
            fs::remove_file(target).map_err(|error| {
                format!(
                    "Failed to remove invalid file {}: {error}",
                    target.display()
                )
            })?;
        }
        if let Some(backup) = backup {
            fs::rename(backup, target).map_err(|error| {
                format!(
                    "Failed to restore original file from {} to {}: {error}",
                    backup.display(),
                    target.display()
                )
            })?;
        }
        Ok(())
    }

    fn write_text_file(
        target: &Path,
        content: &str,
        relative_display: &Path,
    ) -> Result<usize, String> {
        let mut file = fs::File::create(target).map_err(|error| {
            format!(
                "Failed to create file {}: {error}",
                relative_display.display()
            )
        })?;
        file.write_all(content.as_bytes()).map_err(|error| {
            format!(
                "Failed to write file {}: {error}",
                relative_display.display()
            )
        })?;
        Ok(Self::line_count(content))
    }

    fn restore_file_from_content(
        target: &Path,
        relative_display: &Path,
        original_content: &str,
    ) -> Result<(), String> {
        Self::write_text_file(target, original_content, relative_display).map(|_| ())
    }

    fn validate_rust_file(target: &Path, relative_display: &Path) -> Result<(), String> {
        let rustfmt_program = Self::rustfmt_program();
        match Self::run_rustfmt(&rustfmt_program, target)? {
            RustfmtOutcome::NotFound => {
                tracing::warn!(path = %relative_display.display(), "rustfmt not found; writing Rust file without validation");
                Ok(())
            }
            RustfmtOutcome::Failure(details) => Err(format!(
                "Syntax error in {}:\n{details}",
                relative_display.display()
            )),
            RustfmtOutcome::Success => Ok(()),
        }
    }

    fn line_start_offsets(content: &str) -> Vec<usize> {
        let mut offsets = vec![0];
        for (index, byte) in content.bytes().enumerate() {
            if byte == b'\n' {
                offsets.push(index + 1);
            }
        }
        offsets
    }

    fn byte_offset_to_line_index(line_offsets: &[usize], byte_offset: usize) -> usize {
        line_offsets
            .partition_point(|start| *start <= byte_offset)
            .saturating_sub(1)
    }

    fn byte_span_to_line_range(line_offsets: &[usize], start: usize, end: usize) -> (usize, usize) {
        let start_line = Self::byte_offset_to_line_index(line_offsets, start);
        if start == end {
            return (start_line, start_line);
        }
        let end_line = Self::byte_offset_to_line_index(line_offsets, end.saturating_sub(1)) + 1;
        (start_line, end_line)
    }

    fn find_match_offsets(content: &str, needle: &str) -> Vec<usize> {
        content
            .match_indices(needle)
            .map(|(index, _)| index)
            .collect()
    }

    fn build_prepared_edit(
        relative_path: &Path,
        old_content: String,
        old_string: String,
        new_string: String,
        replace_all: bool,
    ) -> Result<PreparedEdit, String> {
        if old_string.is_empty() {
            return Err("old_string must not be empty.".to_owned());
        }

        let match_offsets = Self::find_match_offsets(&old_content, &old_string);
        if match_offsets.is_empty() {
            return Err(format!(
                "old_string not found in {}.",
                relative_path.display()
            ));
        }
        if !replace_all && match_offsets.len() > 1 {
            return Err(format!(
                "old_string matches {} locations in {}. Provide more surrounding context to make the match unique.",
                match_offsets.len(),
                relative_path.display()
            ));
        }

        let mut new_content = String::with_capacity(
            old_content.len()
                + match_offsets.len() * new_string.len().saturating_sub(old_string.len()),
        );
        let mut occurrences = Vec::with_capacity(match_offsets.len());
        let mut old_cursor = 0;
        let mut new_cursor = 0;

        for old_start in match_offsets {
            let old_end = old_start + old_string.len();
            let unchanged = &old_content[old_cursor..old_start];
            new_content.push_str(unchanged);
            new_cursor += unchanged.len();

            let new_start = new_cursor;
            new_content.push_str(&new_string);
            new_cursor += new_string.len();
            occurrences.push(ReplacementOccurrence {
                old_start,
                old_end,
                new_start,
                new_end: new_cursor,
            });
            old_cursor = old_end;

            if !replace_all {
                break;
            }
        }

        new_content.push_str(&old_content[old_cursor..]);
        let diff = Self::render_prepared_edit_diff(
            relative_path,
            &old_content,
            &new_content,
            &occurrences,
        );

        Ok(PreparedEdit {
            pending: PendingEdit {
                path: relative_path.to_path_buf(),
                old_content,
                new_content,
                old_string,
                new_string,
                replace_all,
            },
            diff,
        })
    }

    fn render_prepared_edit_diff(
        relative_path: &Path,
        old_content: &str,
        new_content: &str,
        occurrences: &[ReplacementOccurrence],
    ) -> String {
        const CONTEXT_LINES: usize = 3;

        let old_lines = old_content.lines().collect::<Vec<_>>();
        let new_lines = new_content.lines().collect::<Vec<_>>();
        let old_offsets = Self::line_start_offsets(old_content);
        let new_offsets = Self::line_start_offsets(new_content);

        let mut hunks: Vec<(usize, usize, usize, usize)> = Vec::new();
        for occurrence in occurrences {
            let (old_start_line, old_end_line) = Self::byte_span_to_line_range(
                &old_offsets,
                occurrence.old_start,
                occurrence.old_end,
            );
            let (new_start_line, new_end_line) = Self::byte_span_to_line_range(
                &new_offsets,
                occurrence.new_start,
                occurrence.new_end,
            );

            if let Some(previous) = hunks.last_mut() {
                let previous_old_context_end = previous.1.saturating_add(CONTEXT_LINES);
                let previous_new_context_end = previous.3.saturating_add(CONTEXT_LINES);
                if old_start_line <= previous_old_context_end
                    || new_start_line <= previous_new_context_end
                {
                    previous.1 = previous.1.max(old_end_line);
                    previous.3 = previous.3.max(new_end_line);
                    continue;
                }
            }
            hunks.push((old_start_line, old_end_line, new_start_line, new_end_line));
        }

        let mut rendered = vec![
            format!("--- {}", relative_path.display()),
            format!("+++ {}", relative_path.display()),
        ];

        for (old_start_line, old_end_line, new_start_line, new_end_line) in hunks {
            let old_context_start = old_start_line.saturating_sub(CONTEXT_LINES);
            let new_context_start = new_start_line.saturating_sub(CONTEXT_LINES);
            let old_context_end = old_lines.len().min(old_end_line + CONTEXT_LINES);
            let new_context_end = new_lines.len().min(new_end_line + CONTEXT_LINES);

            rendered.push(format!(
                "@@ -{},{} +{},{} @@",
                old_context_start + 1,
                old_context_end.saturating_sub(old_context_start),
                new_context_start + 1,
                new_context_end.saturating_sub(new_context_start),
            ));

            for line in &old_lines[old_context_start..old_start_line] {
                rendered.push(format!(" {line}"));
            }
            for line in &old_lines[old_start_line..old_end_line] {
                rendered.push(format!("-{line}"));
            }
            for line in &new_lines[new_start_line..new_end_line] {
                rendered.push(format!("+{line}"));
            }

            let old_suffix = old_lines.get(old_end_line..old_context_end).unwrap_or(&[]);
            for line in old_suffix {
                rendered.push(format!(" {line}"));
            }
        }

        rendered.join("\n")
    }

    fn format_read_file_excerpt(
        path: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<String, String> {
        let mut binary_probe =
            fs::File::open(path).map_err(|error| format!("Failed to read file: {error}"))?;
        let mut probe = vec![0; BINARY_DETECTION_BYTES];
        let probe_len = binary_probe
            .read(&mut probe)
            .map_err(|error| format!("Failed to read file: {error}"))?;
        if probe[..probe_len].contains(&0) {
            return Err("Binary file — cannot display.".to_owned());
        }

        let file = fs::File::open(path).map_err(|error| format!("Failed to read file: {error}"))?;
        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        for line in reader.lines() {
            lines.push(line.map_err(|error| format!("Failed to read file: {error}"))?);
        }

        if lines.is_empty() {
            return Ok("File is empty.".to_owned());
        }

        let total = lines.len();
        if offset > total {
            return Err(format!(
                "Offset {offset} is past end of file ({total} lines)."
            ));
        }

        let start_index = offset - 1;
        let end_index = total.min(start_index.saturating_add(limit));
        let width = end_index.to_string().len();
        let mut rendered = Vec::with_capacity(end_index - start_index + 1);
        for (line_number, line) in lines[start_index..end_index].iter().enumerate() {
            let line_number = start_index + line_number + 1;
            let display = if line.chars().count() > MAX_READ_FILE_LINE_LENGTH {
                let truncated: String = line.chars().take(MAX_READ_FILE_LINE_LENGTH).collect();
                format!("{truncated}…")
            } else {
                line.clone()
            };
            rendered.push(format!("{line_number:>width$}→{display}"));
        }

        if end_index < total {
            rendered.push(format!(
                "(truncated — file has {total} lines, showing {offset}–{end_index}. Use offset/limit to read more.)"
            ));
        }

        Ok(rendered.join("\n"))
    }

    // r[mate.tool.run-command]
    async fn mate_tool_run_command(
        &self,
        session_id: &SessionId,
        command: String,
        cwd: Option<String>,
    ) -> Result<String, String> {
        if Self::is_dangerous_command(&command) {
            return Err(RUN_COMMAND_GUARDRAIL_TEMPLATE.replace("{command}", &command));
        }

        let relative_cwd = match cwd {
            Some(cwd) => Some(Self::validate_worktree_path(&cwd)?.to_path_buf()),
            None => None,
        };
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            Self::require_mate_plan(session)?;
            Self::current_task_worktree_path(session)?.to_path_buf()
        };

        let resolved_cwd = tokio::task::spawn_blocking(move || {
            let canonical_worktree = fs::canonicalize(&worktree_path).map_err(|error| {
                format!(
                    "Failed to resolve worktree path {}: {error}",
                    worktree_path.display()
                )
            })?;

            match relative_cwd {
                Some(relative_cwd) => {
                    Self::resolve_worktree_directory(&canonical_worktree, &relative_cwd)
                }
                None => Ok(canonical_worktree),
            }
        })
        .await
        .map_err(|error| format!("run_command path resolution failed: {error}"))??;

        let shell_command = format!("exec 2>&1; {}", command);
        let mut child = Self::sandboxed_sh(&resolved_cwd, &shell_command)?;

        let output = match tokio::time::timeout(RUN_COMMAND_TIMEOUT, child.wait_with_output()).await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => return Err(format!("Command execution failed: {error}")),
            Err(_) => return Err("Command timed out after 120 seconds.".to_owned()),
        };

        let combined_output = String::from_utf8_lossy(&output.stdout);
        let truncated = Self::truncate_run_command_output(&combined_output);
        let exit_code = output.status.code().map_or_else(
            || "terminated by signal".to_owned(),
            |code| code.to_string(),
        );

        if truncated.is_empty() {
            Ok(format!("exit code: {exit_code}"))
        } else {
            Ok(format!("{truncated}\nexit code: {exit_code}"))
        }
    }

    // r[ui.composer.file-mention]
    async fn expand_file_mentions(
        &self,
        session_id: &SessionId,
        parts: Vec<PromptContentPart>,
    ) -> Vec<PromptContentPart> {
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let Some(session) = sessions.get(session_id) else {
                return parts;
            };
            match session.worktree_path.clone() {
                Some(p) => p,
                None => return parts,
            }
        };
        let parts_clone = parts.clone();
        tokio::task::spawn_blocking(move || {
            parts_clone
                .into_iter()
                .map(|part| match part {
                    PromptContentPart::Text { text } => PromptContentPart::Text {
                        text: Self::expand_file_mentions_sync(&worktree_path, &text),
                    },
                    image @ PromptContentPart::Image { .. } => image,
                })
                .collect()
        })
        .await
        .unwrap_or(parts)
    }

    fn expand_file_mentions_sync(worktree: &Path, content: &str) -> String {
        let mut result = String::with_capacity(content.len());
        let mut chars = content.char_indices().peekable();
        while let Some((_i, c)) = chars.next() {
            if c != '@' {
                result.push(c);
                continue;
            }
            let mut path = String::new();
            loop {
                match chars.peek() {
                    Some((_, ch)) if matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '/' | '.' | '_' | '-') =>
                    {
                        path.push(chars.next().unwrap().1);
                    }
                    _ => break,
                }
            }
            if path.is_empty() {
                result.push('@');
                continue;
            }
            match Self::read_file_for_mention(worktree, &path) {
                Ok(file_content) => {
                    let lang = Path::new(&path)
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("");
                    result.push_str(&format!("@{path}:\n```{lang}\n{file_content}\n```"));
                }
                Err(_) => {
                    result.push('@');
                    result.push_str(&path);
                }
            }
        }
        result
    }

    fn read_file_for_mention(worktree: &Path, path_str: &str) -> Result<String, String> {
        let rel_path = Self::validate_worktree_path(path_str)?;
        let canonical_worktree =
            fs::canonicalize(worktree).map_err(|e| format!("worktree canonicalize: {e}"))?;
        let candidate = canonical_worktree.join(rel_path);
        let metadata = fs::metadata(&candidate).map_err(|_| "not found".to_owned())?;
        if metadata.is_dir() {
            return Err("is a directory".to_owned());
        }
        let canonical_file =
            fs::canonicalize(&candidate).map_err(|_| "canonicalize failed".to_owned())?;
        if !canonical_file.starts_with(&canonical_worktree) {
            return Err("path escapes worktree".to_owned());
        }
        let mut f = fs::File::open(&canonical_file).map_err(|e| format!("open: {e}"))?;
        let mut probe = vec![0u8; BINARY_DETECTION_BYTES];
        let probe_len = f.read(&mut probe).map_err(|e| format!("probe read: {e}"))?;
        if probe[..probe_len].contains(&0) {
            return Err("binary file".to_owned());
        }
        let f2 = fs::File::open(&canonical_file).map_err(|e| format!("open: {e}"))?;
        let reader = BufReader::new(f2);
        let mut lines = Vec::new();
        let mut truncated = false;
        for line in reader.lines() {
            if lines.len() >= FILE_MENTION_LINE_LIMIT {
                truncated = true;
                break;
            }
            lines.push(line.map_err(|e| format!("read line: {e}"))?);
        }
        let mut file_content = lines.join("\n");
        if truncated {
            file_content.push_str(&format!(
                "\n(truncated — showing first {FILE_MENTION_LINE_LIMIT} lines)"
            ));
        }
        Ok(file_content)
    }

    async fn list_worktree_files_impl(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<String>, String> {
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            Self::current_task_worktree_path(session)?.to_path_buf()
        };
        let output = TokioCommand::new(Self::fd_program())
            .args(["--type", "f"])
            .current_dir(&worktree_path)
            .output()
            .await
            .map_err(|e| format!("fd failed: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .take(MAX_WORKTREE_FILES)
            .map(|s| s.to_owned())
            .collect())
    }

    // r[mate.tool.read-file]
    async fn mate_tool_read_file(
        &self,
        session_id: &SessionId,
        path: String,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> Result<String, String> {
        let offset = match offset {
            Some(0) => return Err("offset must be at least 1".to_owned()),
            Some(offset) => {
                usize::try_from(offset).map_err(|_| "offset is too large".to_owned())?
            }
            None => 1,
        };
        let limit = match limit {
            Some(0) => return Err("limit must be at least 1".to_owned()),
            Some(limit) => usize::try_from(limit).map_err(|_| "limit is too large".to_owned())?,
            None => DEFAULT_READ_FILE_LIMIT,
        };
        let candidate = Path::new(&path);
        if candidate.is_absolute() {
            // Absolute paths are allowed for read_file (read-only access).
            // The agent may need to read files outside the worktree, e.g. installed
            // crate sources in ~/.cargo/registry.
            let candidate_path = candidate.to_path_buf();
            return tokio::task::spawn_blocking(move || {
                let metadata = fs::metadata(&candidate_path).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::NotFound {
                        format!("File not found: {}", candidate_path.display())
                    } else {
                        format!("Failed to access file: {error}")
                    }
                })?;
                if metadata.is_dir() {
                    return Err("Path is a directory, not a file.".to_owned());
                }
                let canonical_file = fs::canonicalize(&candidate_path).map_err(|error| {
                    format!(
                        "Failed to resolve file path {}: {error}",
                        candidate_path.display()
                    )
                })?;
                Self::format_read_file_excerpt(&canonical_file, offset, limit)
            })
            .await
            .map_err(|error| format!("read_file task failed: {error}"))?;
        }

        let relative_path = Self::validate_worktree_path(&path)?.to_path_buf();
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            Self::current_task_worktree_path(session)?.to_path_buf()
        };

        tokio::task::spawn_blocking(move || {
            let canonical_worktree = fs::canonicalize(&worktree_path).map_err(|error| {
                format!(
                    "Failed to resolve worktree path {}: {error}",
                    worktree_path.display()
                )
            })?;
            let candidate_path = canonical_worktree.join(&relative_path);
            let metadata = fs::metadata(&candidate_path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    format!("File not found: {}", relative_path.display())
                } else {
                    format!("Failed to access file: {error}")
                }
            })?;
            if metadata.is_dir() {
                return Err("Path is a directory, not a file.".to_owned());
            }

            let canonical_file = fs::canonicalize(&candidate_path).map_err(|error| {
                format!(
                    "Failed to resolve file path {}: {error}",
                    candidate_path.display()
                )
            })?;
            if !canonical_file.starts_with(&canonical_worktree) {
                return Err("Path resolves outside the worktree.".to_owned());
            }

            Self::format_read_file_excerpt(&canonical_file, offset, limit)
        })
        .await
        .map_err(|error| format!("read_file task failed: {error}"))?
    }

    // r[mate.tool.write-file]
    async fn mate_tool_write_file(
        &self,
        session_id: &SessionId,
        path: String,
        content: String,
    ) -> McpToolCallResponse {
        let relative_path = match Self::validate_worktree_path(&path) {
            Ok(p) => p.to_path_buf(),
            Err(e) => {
                return McpToolCallResponse {
                    text: e,
                    is_error: true,
                    diffs: vec![],
                };
            }
        };
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = match sessions.get(session_id) {
                Some(s) => s,
                None => {
                    return McpToolCallResponse {
                        text: format!("session not found: {}", session_id.0),
                        is_error: true,
                        diffs: vec![],
                    };
                }
            };
            match Self::require_mate_plan(session) {
                Ok(()) => {}
                Err(e) => {
                    return McpToolCallResponse {
                        text: e,
                        is_error: true,
                        diffs: vec![],
                    };
                }
            }
            match Self::current_task_worktree_path(session) {
                Ok(p) => p.to_path_buf(),
                Err(e) => {
                    return McpToolCallResponse {
                        text: e,
                        is_error: true,
                        diffs: vec![],
                    };
                }
            }
        };

        let result = tokio::task::spawn_blocking({
            let relative_path = relative_path.clone();
            move || {
                let canonical_worktree = fs::canonicalize(&worktree_path).map_err(|error| {
                    format!(
                        "Failed to resolve worktree path {}: {error}",
                        worktree_path.display()
                    )
                })?;
                let target_path =
                    Self::resolve_worktree_file_path(&canonical_worktree, &relative_path)?;

                if let Ok(metadata) = fs::metadata(&target_path) {
                    if metadata.is_dir() {
                        return Err("Path is a directory, not a file.".to_owned());
                    }
                }

                let old_text = fs::read_to_string(&target_path).ok();

                let line_count = if relative_path.extension().and_then(|ext| ext.to_str())
                    == Some("rs")
                {
                    let backup_path = if target_path.exists() {
                        let backup_name = format!(
                            "{}.ship-backup-{}",
                            target_path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .ok_or_else(|| "Path must point to a file.".to_owned())?,
                            std::process::id()
                        );
                        let backup = target_path.with_file_name(backup_name);
                        fs::rename(&target_path, &backup).map_err(|error| {
                            format!(
                                "Failed to back up existing file {}: {error}",
                                relative_path.display()
                            )
                        })?;
                        Some(backup)
                    } else {
                        None
                    };

                    let write_result =
                        Self::write_text_file(&target_path, &content, &relative_path);
                    let line_count = match write_result {
                        Ok(line_count) => line_count,
                        Err(error) => {
                            if let Some(backup) = backup_path.as_deref() {
                                let _ = fs::rename(backup, &target_path);
                            }
                            return Err(error);
                        }
                    };

                    if let Err(error) = Self::validate_rust_file(&target_path, &relative_path) {
                        let restore_error =
                            Self::restore_written_file(&target_path, backup_path.as_deref());
                        return match restore_error {
                            Ok(()) => Err(error),
                            Err(restore_error) => Err(format!("{error}\n{restore_error}")),
                        };
                    }

                    if let Some(backup) = backup_path {
                        fs::remove_file(&backup).map_err(|error| {
                            format!("Failed to remove backup file {}: {error}", backup.display())
                        })?;
                    }

                    line_count
                } else {
                    Self::write_text_file(&target_path, &content, &relative_path)?
                };

                let path_str = relative_path.display().to_string();
                Ok((
                    format!("Wrote {path_str} ({line_count} lines)"),
                    McpDiffContent {
                        path: path_str,
                        old_text,
                        new_text: content,
                        edit_id: None,
                    },
                ))
            }
        })
        .await;

        match result {
            Ok(Ok((text, diff))) => McpToolCallResponse {
                text,
                is_error: false,
                diffs: vec![diff],
            },
            Ok(Err(text)) => McpToolCallResponse {
                text,
                is_error: true,
                diffs: vec![],
            },
            Err(error) => McpToolCallResponse {
                text: format!("write_file task failed: {error}"),
                is_error: true,
                diffs: vec![],
            },
        }
    }

    // r[mate.tool.edit-prepare]
    async fn mate_tool_edit_prepare(
        &self,
        session_id: &SessionId,
        path: String,
        old_string: String,
        new_string: String,
        replace_all: Option<bool>,
    ) -> McpToolCallResponse {
        let result: Result<McpToolCallResponse, String> = async {
            let relative_path = Self::validate_worktree_path(&path)?.to_path_buf();
            let worktree_path = {
                let sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let session = sessions
                    .get(session_id)
                    .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                Self::require_mate_plan(session)?;
                Self::current_task_worktree_path(session)?.to_path_buf()
            };

            let prepared = tokio::task::spawn_blocking({
                let relative_path = relative_path.clone();
                move || {
                    let canonical_worktree = fs::canonicalize(&worktree_path).map_err(|error| {
                        format!(
                            "Failed to resolve worktree path {}: {error}",
                            worktree_path.display()
                        )
                    })?;
                    let candidate_path = canonical_worktree.join(&relative_path);
                    let metadata = fs::metadata(&candidate_path).map_err(|error| {
                        if error.kind() == std::io::ErrorKind::NotFound {
                            format!("File not found: {}", relative_path.display())
                        } else {
                            format!("Failed to access file: {error}")
                        }
                    })?;
                    if metadata.is_dir() {
                        return Err("Path is a directory, not a file.".to_owned());
                    }

                    let canonical_file = fs::canonicalize(&candidate_path).map_err(|error| {
                        format!(
                            "Failed to resolve file path {}: {error}",
                            candidate_path.display()
                        )
                    })?;
                    if !canonical_file.starts_with(&canonical_worktree) {
                        return Err("Path resolves outside the worktree.".to_owned());
                    }

                    let old_content = fs::read_to_string(&canonical_file)
                        .map_err(|error| format!("Failed to read file: {error}"))?;
                    let mut prepared = Self::build_prepared_edit(
                        &relative_path,
                        old_content.clone(),
                        old_string,
                        new_string,
                        replace_all.unwrap_or(false),
                    )?;

                    // Write speculatively, run rustfmt, then restore — this
                    // validates that the edit produces syntactically valid code
                    // before the agent is asked to confirm it.
                    if canonical_file.extension().and_then(|e| e.to_str()) == Some("rs") {
                        Self::write_text_file(
                            &canonical_file,
                            &prepared.pending.new_content,
                            &relative_path,
                        )
                        .map_err(|e| format!("edit_prepare speculative write failed: {e}"))?;

                        let rustfmt_result =
                            Self::run_rustfmt(&Self::rustfmt_program(), &canonical_file);

                        match rustfmt_result {
                            Ok(RustfmtOutcome::Failure(details)) => {
                                let _ = Self::restore_file_from_content(
                                    &canonical_file,
                                    &relative_path,
                                    &old_content,
                                );
                                return Err(format!(
                                    "Edit would produce invalid Rust in {}:\n{details}",
                                    relative_path.display()
                                ));
                            }
                            Ok(RustfmtOutcome::Success) => {
                                // Read the rustfmt-formatted content BEFORE restoring,
                                // so edit_confirm applies the formatted version.
                                let formatted = fs::read_to_string(&canonical_file)
                                    .map_err(|e| format!("Failed to read formatted file: {e}"))?;
                                Self::restore_file_from_content(
                                    &canonical_file,
                                    &relative_path,
                                    &old_content,
                                )
                                .map_err(|e| format!("edit_prepare restore failed: {e}"))?;
                                prepared.pending.new_content = formatted;
                            }
                            Ok(RustfmtOutcome::NotFound) => {
                                Self::restore_file_from_content(
                                    &canonical_file,
                                    &relative_path,
                                    &old_content,
                                )
                                .map_err(|e| format!("edit_prepare restore failed: {e}"))?;
                            }
                            Err(e) => {
                                let _ = Self::restore_file_from_content(
                                    &canonical_file,
                                    &relative_path,
                                    &old_content,
                                );
                                return Err(format!("edit_prepare rustfmt failed: {e}"));
                            }
                        }
                    }

                    Ok(prepared)
                }
            })
            .await
            .map_err(|error| format!("edit_prepare task failed: {error}"))??;

            let edit_id = format!("edit-{}", ulid::Ulid::new());
            let old_content = prepared.pending.old_content.clone();
            let new_content = prepared.pending.new_content.clone();
            let path_str = relative_path.display().to_string();
            {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let session = sessions
                    .get_mut(session_id)
                    .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                session
                    .pending_edits
                    .insert(edit_id.clone(), prepared.pending);
            }

            Ok(McpToolCallResponse {
                text: format!(
                    "Edit prepared. Pass this edit_id to edit_confirm to apply it: {edit_id}"
                ),
                is_error: false,
                diffs: vec![McpDiffContent {
                    path: path_str,
                    old_text: Some(old_content),
                    new_text: new_content,
                    edit_id: Some(edit_id),
                }],
            })
        }
        .await;
        match result {
            Ok(r) => r,
            Err(e) => McpToolCallResponse {
                text: e,
                is_error: true,
                diffs: vec![],
            },
        }
    }

    // r[mate.tool.edit-confirm]
    async fn mate_tool_edit_confirm(
        &self,
        session_id: &SessionId,
        edit_id: String,
    ) -> McpToolCallResponse {
        let (worktree_path, pending_edit) = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = match sessions.get(session_id) {
                Some(s) => s,
                None => {
                    return McpToolCallResponse {
                        text: format!("session not found: {}", session_id.0),
                        is_error: true,
                        diffs: vec![],
                    };
                }
            };
            match Self::require_mate_plan(session) {
                Ok(()) => {}
                Err(e) => {
                    return McpToolCallResponse {
                        text: e,
                        is_error: true,
                        diffs: vec![],
                    };
                }
            }
            let pending_edit = match session.pending_edits.get(&edit_id).cloned() {
                Some(e) => e,
                None => {
                    return McpToolCallResponse {
                        text: "edit_id not found. It may have expired or been superseded."
                            .to_owned(),
                        is_error: true,
                        diffs: vec![],
                    };
                }
            };
            let worktree = match Self::current_task_worktree_path(session) {
                Ok(p) => p.to_path_buf(),
                Err(e) => {
                    return McpToolCallResponse {
                        text: e,
                        is_error: true,
                        diffs: vec![],
                    };
                }
            };
            (worktree, pending_edit)
        };

        let confirmed_path = pending_edit.path.clone();
        let result = tokio::task::spawn_blocking(move || {
            let canonical_worktree = fs::canonicalize(&worktree_path).map_err(|error| {
                format!(
                    "Failed to resolve worktree path {}: {error}",
                    worktree_path.display()
                )
            })?;
            let target_path = canonical_worktree.join(&pending_edit.path);
            let metadata = fs::metadata(&target_path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    format!("File not found: {}", pending_edit.path.display())
                } else {
                    format!("Failed to access file: {error}")
                }
            })?;
            if metadata.is_dir() {
                return Err("Path is a directory, not a file.".to_owned());
            }

            let canonical_file = fs::canonicalize(&target_path).map_err(|error| {
                format!(
                    "Failed to resolve file path {}: {error}",
                    target_path.display()
                )
            })?;
            if !canonical_file.starts_with(&canonical_worktree) {
                return Err("Path resolves outside the worktree.".to_owned());
            }

            let current_content = fs::read_to_string(&canonical_file)
                .map_err(|error| format!("Failed to read file: {error}"))?;
            let (old_before_write, new_content) = if current_content == pending_edit.old_content {
                (
                    pending_edit.old_content.clone(),
                    pending_edit.new_content.clone(),
                )
            } else {
                // File changed since prepare (likely another edit in the same
                // batch was confirmed first). Re-apply the transformation to
                // the current content rather than failing.
                let reapplied = Self::build_prepared_edit(
                    &pending_edit.path,
                    current_content.clone(),
                    pending_edit.old_string.clone(),
                    pending_edit.new_string.clone(),
                    pending_edit.replace_all,
                )
                .map_err(|error| {
                    format!("File changed since edit was prepared and re-apply failed: {error}")
                })?;
                (current_content, reapplied.pending.new_content)
            };

            Self::write_text_file(&canonical_file, &new_content, &pending_edit.path)?;
            if pending_edit.path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                if let Err(error) = Self::validate_rust_file(&canonical_file, &pending_edit.path) {
                    let restore_error = Self::restore_file_from_content(
                        &canonical_file,
                        &pending_edit.path,
                        &old_before_write,
                    );
                    return match restore_error {
                        Ok(()) => Err(error),
                        Err(restore_error) => Err(format!("{error}\n{restore_error}")),
                    };
                }
            }

            Ok((old_before_write, new_content))
        })
        .await;

        match result {
            Ok(Ok((old_content, new_content))) => {
                {
                    let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    if let Some(session) = sessions.get_mut(session_id) {
                        session.pending_edits.remove(&edit_id);
                    }
                }
                let path_str = confirmed_path.display().to_string();
                McpToolCallResponse {
                    text: format!("Applied {edit_id} to {path_str}"),
                    is_error: false,
                    diffs: vec![McpDiffContent {
                        path: path_str,
                        old_text: Some(old_content),
                        new_text: new_content,
                        edit_id: None,
                    }],
                }
            }
            Ok(Err(text)) => McpToolCallResponse {
                text,
                is_error: true,
                diffs: vec![],
            },
            Err(error) => McpToolCallResponse {
                text: format!("edit_confirm task failed: {error}"),
                is_error: true,
                diffs: vec![],
            },
        }
    }

    // r[mate.tool.search-files]
    async fn mate_tool_search_files(
        &self,
        session_id: &SessionId,
        args: String,
    ) -> Result<String, String> {
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            Self::current_task_worktree_path(session)?.to_path_buf()
        };

        Self::run_worktree_shell_command(
            worktree_path,
            Self::rg_program(),
            args,
            "ripgrep (rg) is not installed.",
            Some("No matches found."),
        )
        .await
    }

    // r[mate.tool.list-files]
    async fn mate_tool_list_files(
        &self,
        session_id: &SessionId,
        args: String,
    ) -> Result<String, String> {
        let worktree_path = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            Self::current_task_worktree_path(session)?.to_path_buf()
        };

        Self::run_worktree_shell_command(
            worktree_path,
            Self::fd_program(),
            args,
            "fd is not installed.",
            None,
        )
        .await
    }

    async fn auto_commit_worktree(
        worktree_path: &std::path::Path,
        message: String,
    ) -> Result<Option<AutoCommitResult>, String> {
        let worktree_path = worktree_path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let add_status = Command::new("git")
                .arg("-C")
                .arg(&worktree_path)
                .args(["add", "-A"])
                .status()
                .map_err(|error| format!("git add failed: {error}"))?;
            if !add_status.success() {
                return Err("git add -A failed".to_owned());
            }

            let diff_status = Command::new("git")
                .arg("-C")
                .arg(&worktree_path)
                .args(["diff", "--cached", "--quiet"])
                .status()
                .map_err(|error| format!("git diff --cached --quiet failed: {error}"))?;
            if diff_status.success() {
                return Ok(None);
            }

            let commit_output = Command::new("git")
                .arg("-C")
                .arg(&worktree_path)
                .args(["commit", "-m", &message])
                .output()
                .map_err(|error| format!("git commit failed: {error}"))?;
            if !commit_output.status.success() {
                let stderr = String::from_utf8_lossy(&commit_output.stderr)
                    .trim()
                    .to_owned();
                return Err(if stderr.is_empty() {
                    "git commit failed".to_owned()
                } else {
                    format!("git commit failed: {stderr}")
                });
            }

            let commit_hash = Command::new("git")
                .arg("-C")
                .arg(&worktree_path)
                .args(["rev-parse", "HEAD"])
                .output()
                .map_err(|error| format!("git rev-parse HEAD failed: {error}"))?;
            if !commit_hash.status.success() {
                return Err("git rev-parse HEAD failed".to_owned());
            }
            let commit_hash = String::from_utf8_lossy(&commit_hash.stdout)
                .trim()
                .to_owned();

            let diff_stat = Command::new("git")
                .arg("-C")
                .arg(&worktree_path)
                .args(["show", "--stat", "--format=", "--shortstat", "HEAD"])
                .output()
                .map_err(|error| format!("git show --stat failed: {error}"))?;
            if !diff_stat.status.success() {
                return Err("git show --stat failed".to_owned());
            }

            Ok(Some(AutoCommitResult {
                commit_hash,
                diff_stat: String::from_utf8_lossy(&diff_stat.stdout).trim().to_owned(),
            }))
        })
        .await
        .map_err(|error| format!("git auto-commit task failed: {error}"))?
    }

    // r[task.progress]
    async fn notify_captain_progress(
        &self,
        session_id: &SessionId,
        message: String,
    ) -> Result<(), String> {
        let wrapped = format!("<system-notification>\n{message}\n</system-notification>");
        self.append_human_message(
            session_id,
            Role::Captain,
            &[PromptContentPart::Text {
                text: wrapped.clone(),
            }],
        )
        .await?;

        let this = self.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this.interrupt_captain(&session_id, wrapped).await {
                Self::log_error("notify_captain_progress", &error);
            }
        });

        Ok(())
    }

    /// Append a progress message to the captain's feed without interrupting.
    /// Use this when the mate wants the captain to see progress but the captain
    /// should NOT be prompted to respond yet (e.g. plan step completions).
    async fn append_captain_feed(
        &self,
        session_id: &SessionId,
        message: String,
    ) -> Result<(), String> {
        let wrapped = format!("<system-notification>\n{message}\n</system-notification>");
        self.append_human_message(
            session_id,
            Role::Captain,
            &[PromptContentPart::Text { text: wrapped }],
        )
        .await
    }

    fn commit_summary(result: Option<&AutoCommitResult>) -> String {
        match result {
            Some(result) if result.diff_stat.is_empty() => {
                format!("Commit: {}", result.commit_hash)
            }
            Some(result) => format!("Commit: {}\nDiff: {}", result.commit_hash, result.diff_stat),
            None => "Commit: skipped (worktree clean)".to_owned(),
        }
    }

    // r[mate.tool.plan-create]
    // Checks that the mate has created a plan before allowing substantive work.
    // Returns Ok(()) if a plan exists, Err with a human-readable message otherwise.
    fn require_mate_plan(session: &ActiveSession) -> Result<(), String> {
        let has_plan = session
            .current_task
            .as_ref()
            .is_some_and(|task| task.mate_plan.is_some());
        if has_plan {
            Ok(())
        } else {
            Err(PLAN_REQUIRED_MESSAGE.to_owned())
        }
    }

    fn queue_mate_guidance(session: &mut ActiveSession, message: &str) {
        if let Some(task) = session.current_task.as_mut() {
            task.pending_mate_guidance = Some(message.to_owned());
        }
    }

    fn take_pending_mate_guidance(session: &mut ActiveSession) -> Option<String> {
        session
            .current_task
            .as_mut()
            .and_then(|task| task.pending_mate_guidance.take())
    }

    async fn mate_tool_send_update(
        &self,
        session_id: &SessionId,
        message: String,
    ) -> Result<String, String> {
        // Inject the update into the captain's stream as a user message, then prompt the captain.
        let injected = format!("The mate sent you an update: {message}");
        self.append_human_message(
            session_id,
            Role::Captain,
            &[PromptContentPart::Text {
                text: injected.clone(),
            }],
        )
        .await?;

        let this = self.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this.interrupt_captain(&session_id, injected).await {
                Self::log_error("mate_send_update prompt_captain", &error);
            }
        });

        Ok("Update sent to the captain.".to_owned())
    }

    // r[mate.tool.plan-create]
    // r[task.progress]
    async fn mate_tool_set_plan(
        &self,
        session_id: &SessionId,
        steps: Vec<String>,
    ) -> Result<String, String> {
        if steps.is_empty() {
            return Err("set_plan requires at least one step".to_owned());
        }

        let new_plan = Self::build_plan_steps(steps);

        // Determine if this is the first plan or a mid-task change.
        let (task_description, old_plan) = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let task = session
                .current_task
                .as_ref()
                .ok_or_else(|| "session has no active task".to_owned())?;
            (task.record.description.clone(), task.mate_plan.clone())
        };

        if old_plan.is_none() {
            // First time — set plan and notify captain non-blocking.
            {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let session = sessions
                    .get_mut(session_id)
                    .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                let task = session
                    .current_task
                    .as_mut()
                    .ok_or_else(|| "session has no active task".to_owned())?;
                task.mate_plan = Some(new_plan.clone());
                task.pending_mate_guidance = None;
                set_agent_state(
                    session,
                    Role::Mate,
                    AgentState::Working {
                        plan: Some(new_plan.clone()),
                        activity: Some("Plan set".to_owned()),
                    },
                );
            }
            self.persist_session(session_id).await?;
            let captain_message = format!(
                "The mate has set their plan.\n\n{}\n\nWe will keep you posted as they progress. You have nothing to do now.",
                Self::format_plan_status(&new_plan),
            );
            self.notify_captain_progress(session_id, captain_message)
                .await?;
            Ok(format!("Plan set for task '{task_description}'."))
        } else {
            // Mid-task plan change — tentatively set new plan and block for captain review.
            let old_plan = old_plan.unwrap();
            {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let session = sessions
                    .get_mut(session_id)
                    .ok_or_else(|| format!("session not found: {}", session_id.0))?;
                let task = session
                    .current_task
                    .as_mut()
                    .ok_or_else(|| "session has no active task".to_owned())?;
                task.mate_plan = Some(new_plan.clone());
                task.pending_mate_guidance = None;
                set_agent_state(
                    session,
                    Role::Mate,
                    AgentState::Working {
                        plan: Some(new_plan.clone()),
                        activity: Some("Awaiting captain review of plan change".to_owned()),
                    },
                );
            }
            self.persist_session(session_id).await?;

            let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
            {
                let mut ops = self
                    .pending_mcp_ops
                    .lock()
                    .expect("pending_mcp_ops mutex poisoned");
                let entry = ops
                    .entry(session_id.clone())
                    .or_insert_with(PendingMcpOps::new);
                entry.plan_change_reply = Some((old_plan.clone(), tx));
            }

            let captain_message = format!(
                "The mate is changing their plan mid-task. Something important may have come up.\n\n\
                Previous plan:\n{}\n\nProposed new plan:\n{}\n\n\
                Call `captain_accept` to approve the new plan and unblock the mate, \
                or `captain_steer` to reject it and redirect them (the old plan will be restored).",
                Self::format_plan_status(&old_plan),
                Self::format_plan_status(&new_plan),
            );
            self.interrupt_captain(session_id, captain_message).await?;

            match rx.await {
                Ok(Ok(())) => Ok(
                    "Plan change accepted by the captain. Continue with your new plan.".to_owned(),
                ),
                Ok(Err(message)) => Err(format!(
                    "Plan change rejected by the captain: {message}. \
                    Your previous plan has been restored. Adjust your approach accordingly."
                )),
                Err(_) => Err("Captain disconnected before responding to plan change.".to_owned()),
            }
        }
    }

    // r[mate.tool.plan-step-complete]
    // r[task.progress]
    async fn mate_tool_plan_step_complete(
        &self,
        session_id: &SessionId,
        step_index: usize,
        summary: String,
    ) -> Result<String, String> {
        let (step_description, worktree_path) = {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let (updated_plan, step_description) = {
                let task = session
                    .current_task
                    .as_mut()
                    .ok_or_else(|| "session has no active task".to_owned())?;
                let plan = task
                    .mate_plan
                    .as_mut()
                    .ok_or_else(|| PLAN_REQUIRED_MESSAGE.to_owned())?;
                let Some(step) = plan.get_mut(step_index) else {
                    return Err(format!("plan step {step_index} does not exist"));
                };
                step.status = PlanStepStatus::Completed;
                let step_description = step.description.clone();
                let updated_plan = plan.clone();
                (updated_plan, step_description)
            };
            set_agent_state(
                session,
                Role::Mate,
                AgentState::Working {
                    plan: Some(updated_plan),
                    activity: Some(format!("Completed step: {step_description}")),
                },
            );
            (
                step_description,
                Self::current_task_worktree_path(session)?.to_path_buf(),
            )
        };

        self.persist_session(session_id).await?;

        let commit =
            Self::auto_commit_worktree(&worktree_path, format!("{step_description}: {summary}"))
                .await?;

        if let Some(commit) = commit {
            let commit_summary = Self::commit_summary(Some(&commit));
            let captain_message = format!(
                "The mate completed a step from their plan.\n\nCompleted: {step_description}\n\n{commit_summary}\n\nWe will notify you when they are done and need your review.",
            );
            self.append_captain_feed(session_id, captain_message)
                .await?;
            Ok(format!(
                "Marked step {} complete. {}",
                step_index + 1,
                commit_summary
            ))
        } else {
            Ok(format!(
                "Marked step {} complete. No file changes (research/investigation step).",
                step_index + 1
            ))
        }
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
        self.append_human_message(
            session_id,
            Role::Captain,
            &[PromptContentPart::Text {
                text: injected.clone(),
            }],
        )
        .await?;

        let this = self.clone();
        let session_id_clone = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this.interrupt_captain(&session_id_clone, injected).await {
                Self::log_error("mate_ask_captain prompt_captain", &error);
            }
        });

        match rx.await {
            Ok(reply) => Ok(reply),
            Err(_) => Err("captain reply channel closed".to_owned()),
        }
    }

    // r[task.completion]
    async fn mate_tool_submit(
        &self,
        session_id: &SessionId,
        summary: String,
    ) -> Result<String, String> {
        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let task = session
                .current_task
                .as_mut()
                .ok_or_else(|| "session has no active task".to_owned())?;
            if task.mate_plan.is_none() {
                task.pending_mate_guidance = Some(PLAN_REQUIRED_MESSAGE.to_owned());
                return Err(PLAN_REQUIRED_MESSAGE.to_owned());
            }
        }

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
        self.append_human_message(
            session_id,
            Role::Captain,
            &[PromptContentPart::Text {
                text: injected.clone(),
            }],
        )
        .await?;

        let this = self.clone();
        let session_id_clone = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this.interrupt_captain(&session_id_clone, injected).await {
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
        let captain_spawn = match captain_result {
            Ok(result) => {
                self.log_startup_step_elapsed(&session_id, "spawn-captain", captain_started_at);
                result
            }
            Err(error) => {
                if let Ok(mate_spawn) = mate_result {
                    let _ = self.agent_driver.kill(&mate_spawn.handle).await;
                }
                self.fail_startup(&session_id, stage, error.message).await;
                return;
            }
        };
        let mate_spawn = match mate_result {
            Ok(result) => {
                self.log_startup_step_elapsed(&session_id, "spawn-mate", mate_started_at);
                result
            }
            Err(error) => {
                let _ = self.agent_driver.kill(&captain_spawn.handle).await;
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
                session.captain_handle = Some(captain_spawn.handle);
                session.mate_handle = Some(mate_spawn.handle);
                apply_event(
                    session,
                    ship_types::SessionEvent::AgentModelChanged {
                        role: Role::Captain,
                        model_id: captain_spawn.model_id,
                        available_models: captain_spawn.available_models,
                    },
                );
                apply_event(
                    session,
                    ship_types::SessionEvent::AgentEffortChanged {
                        role: Role::Captain,
                        effort_config_id: captain_spawn.effort_config_id,
                        effort_value_id: captain_spawn.effort_value_id,
                        available_effort_values: captain_spawn.available_effort_values,
                    },
                );
                apply_event(
                    session,
                    ship_types::SessionEvent::AgentModelChanged {
                        role: Role::Mate,
                        model_id: mate_spawn.model_id,
                        available_models: mate_spawn.available_models,
                    },
                );
                apply_event(
                    session,
                    ship_types::SessionEvent::AgentEffortChanged {
                        role: Role::Mate,
                        effort_config_id: mate_spawn.effort_config_id,
                        effort_value_id: mate_spawn.effort_value_id,
                        available_effort_values: mate_spawn.available_effort_values,
                    },
                );
            }
        }
        let _ = self.persist_session(&session_id).await;

        let stage = SessionStartupStage::GreetingCaptain;
        let _ = self.set_startup_stage(&session_id, stage).await;
        let step_started_at = Instant::now();
        if let Err(error) = self
            .prompt_agent_text(&session_id, Role::Captain, Self::captain_bootstrap_prompt())
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
                created_at: session.created_at.clone(),
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

    fn event_starts_substantive_mate_work(event: &SessionEvent) -> bool {
        let kind = match event {
            SessionEvent::BlockAppend {
                role: Role::Mate,
                block: ContentBlock::ToolCall { kind, .. } | ContentBlock::Permission { kind, .. },
                ..
            } => *kind,
            _ => None,
        };

        matches!(
            kind,
            Some(
                ToolCallKind::Read
                    | ToolCallKind::Edit
                    | ToolCallKind::Delete
                    | ToolCallKind::Move
                    | ToolCallKind::Search
                    | ToolCallKind::Execute
                    | ToolCallKind::Fetch
            )
        )
    }

    fn blocked_command_from_event(event: &SessionEvent) -> Option<String> {
        let target = match event {
            SessionEvent::BlockAppend {
                role: Role::Mate,
                block: ContentBlock::Permission { target, .. },
                ..
            } => target.as_ref(),
            _ => None,
        }?;

        match target {
            ToolTarget::Command { command, .. } if Self::is_dangerous_command(command) => {
                Some(command.clone())
            }
            _ => None,
        }
    }

    fn is_dangerous_command(command: &str) -> bool {
        let normalized = command.trim().to_ascii_lowercase();
        let mut parts = normalized.split_whitespace();
        let Some(program) = parts.next() else {
            return false;
        };
        let subcommand = parts.next();

        if program == "git"
            && matches!(subcommand, Some("checkout" | "restore" | "clean" | "reset"))
        {
            return true;
        }

        if program != "rm" {
            return false;
        }

        let has_recursive = normalized.contains(" -r")
            || normalized.contains(" -rf")
            || normalized.contains(" -fr")
            || normalized.contains(" --recursive");
        let has_force = normalized.contains(" -f")
            || normalized.contains(" -rf")
            || normalized.contains(" -fr")
            || normalized.contains(" --force");
        let broad_target = normalized.contains(" *")
            || normalized.ends_with(" .")
            || normalized.contains(" ./")
            || normalized.contains(" /")
            || normalized.contains(" ..")
            || normalized.contains(" ~");

        has_recursive && has_force && broad_target
    }

    fn inspect_mate_event_for_guardrails(
        session: &mut ActiveSession,
        event: &SessionEvent,
    ) -> Option<String> {
        if Self::event_starts_substantive_mate_work(event)
            && session
                .current_task
                .as_ref()
                .is_some_and(|task| task.mate_plan.is_none())
        {
            Self::queue_mate_guidance(session, PLAN_REQUIRED_MESSAGE);
        }

        let blocked = Self::blocked_command_from_event(event)?;
        Self::queue_mate_guidance(session, BLOCKED_COMMAND_MESSAGE);
        let task_description = session
            .current_task
            .as_ref()
            .map(|task| task.record.description.clone())
            .unwrap_or_else(|| "unknown task".to_owned());
        Some(format!(
            "Task: {task_description}\n\nThe mate attempted a blocked command: {blocked}\n\nThe command was rejected automatically. The mate will be told to stop and explain the situation via mate_ask_captain."
        ))
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
            // MateGuidanceQueued is an internal signal from the ACP client —
            // inject the guidance into the appropriate agent's feed.
            if let SessionEvent::MateGuidanceQueued { role, message } = &event {
                match role {
                    Role::Mate => {
                        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                        if let Some(session) = sessions.get_mut(session_id) {
                            Self::queue_mate_guidance(session, message);
                        }
                    }
                    Role::Captain => {
                        let this = self.clone();
                        let session_id = session_id.clone();
                        let message = message.clone();
                        tokio::spawn(async move {
                            if let Err(error) = this.interrupt_captain(&session_id, message).await {
                                Self::log_error("builtin_tool_blocked_interrupt_captain", &error);
                            }
                        });
                    }
                }
                continue;
            }

            let captain_notification = {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let Some(session) = sessions.get_mut(session_id) else {
                    break;
                };
                let captain_notification = if role == Role::Mate {
                    Self::inspect_mate_event_for_guardrails(session, &event)
                } else {
                    None
                };
                apply_event(session, event);
                captain_notification
            };

            if let Some(message) = captain_notification {
                self.notify_captain_progress(session_id, message).await?;
            }
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

    async fn prompt_agent_text(
        &self,
        session_id: &SessionId,
        role: Role,
        text: String,
    ) -> Result<ship_core::StopReason, String> {
        self.prompt_agent(session_id, role, vec![PromptContentPart::Text { text }])
            .await
    }

    /// Interrupt the captain (cancel any in-flight prompt) and then send a
    /// new prompt. This is the right thing to do when the mate submits,
    /// asks a question, or sends an update — the captain needs to stop what
    /// it's doing and deal with the new information.
    async fn interrupt_captain(
        &self,
        session_id: &SessionId,
        text: String,
    ) -> Result<ship_core::StopReason, String> {
        self.interrupt_captain_with_parts(session_id, vec![PromptContentPart::Text { text }])
            .await
    }

    async fn interrupt_captain_with_parts(
        &self,
        session_id: &SessionId,
        parts: Vec<PromptContentPart>,
    ) -> Result<ship_core::StopReason, String> {
        let handle = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            session
                .captain_handle
                .clone()
                .ok_or_else(|| "captain agent not ready".to_owned())?
        };

        // Cancel any in-flight prompt — this makes the current prompt()
        // call return with StopReason::Cancelled, which resets the
        // prompt_in_flight flag.
        let _ = self.agent_driver.cancel(&handle).await;

        self.prompt_agent(session_id, Role::Captain, parts).await
    }

    async fn prompt_agent(
        &self,
        session_id: &SessionId,
        role: Role,
        parts: Vec<PromptContentPart>,
    ) -> Result<ship_core::StopReason, String> {
        let text_len: usize = parts
            .iter()
            .filter_map(|p| {
                if let PromptContentPart::Text { text } = p {
                    Some(text.len())
                } else {
                    None
                }
            })
            .sum();
        tracing::info!(session_id = %session_id.0, role = ?role, text_len, "starting agent prompt");
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
        let response = match self.agent_driver.prompt(&handle, &parts).await {
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
                // mate_submit may have already transitioned to ReviewPending; only transition
                // if it hasn't (avoids double-transition error)
                let status = current_task_status(session).map_err(|e| e.to_string())?;
                if status != TaskStatus::ReviewPending {
                    transition_task(session, TaskStatus::ReviewPending)
                        .map_err(|error| error.to_string())?;
                }
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
        title: String,
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
                    title: title.clone(),
                    description: description.clone(),
                    status: TaskStatus::Assigned,
                },
                mate_plan: None,
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

    /// Extracts the last few mate text blocks from an event log to build a summary.
    fn build_summary_from_event_log(event_log: &[SessionEventEnvelope]) -> String {
        let mut texts: Vec<String> = Vec::new();
        for envelope in event_log.iter().rev() {
            if let SessionEvent::BlockAppend {
                role: Role::Mate,
                block: ContentBlock::Text { text, .. },
                ..
            } = &envelope.event
            {
                texts.push(text.clone());
                if texts.len() >= 5 {
                    break;
                }
            }
        }
        texts.reverse();
        if texts.is_empty() {
            "No recent output available.".to_owned()
        } else {
            texts.join("\n\n")
        }
    }

    /// Performs a forced mate submission when the mate stopped without calling `mate_submit`.
    /// Sets up the review channel, transitions to ReviewPending, and prompts the captain.
    async fn force_mate_submit(
        &self,
        session_id: &SessionId,
        preamble: &str,
    ) -> Result<(), String> {
        let summary = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("session not found: {}", session_id.0))?;
            let event_log = session
                .current_task
                .as_ref()
                .map(|t| t.event_log.as_slice())
                .unwrap_or(&[]);
            Self::build_summary_from_event_log(event_log)
        };

        // Set up review channel so captain_accept/steer/cancel can complete the review.
        // We don't await rx — the mate is already done, so the tx is just a signal path.
        {
            let mut ops = self
                .pending_mcp_ops
                .lock()
                .expect("pending_mcp_ops mutex poisoned");
            let entry = ops
                .entry(session_id.clone())
                .or_insert_with(PendingMcpOps::new);
            let (tx, _rx) = tokio::sync::oneshot::channel::<MateReviewOutcome>();
            entry.mate_review = Some(tx);
        }

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
            if let Some(active) = sessions.get_mut(session_id) {
                let _ = transition_task(active, TaskStatus::ReviewPending);
            }
        }
        self.persist_session(session_id).await?;

        let injected = format!("{preamble}\n\n{summary}");
        self.append_human_message(
            session_id,
            Role::Captain,
            &[PromptContentPart::Text {
                text: injected.clone(),
            }],
        )
        .await?;

        let this = self.clone();
        let session_id_clone = session_id.clone();
        tokio::spawn(async move {
            if let Err(error) = this.interrupt_captain(&session_id_clone, injected).await {
                Self::log_error("force_mate_submit prompt_captain", &error);
            }
        });

        Ok(())
    }

    async fn prompt_mate_from_steer(&self, session_id: SessionId, parts: Vec<PromptContentPart>) {
        // Prepend "Captain steer:\n" to the text parts for the first prompt.
        let initial_parts: Vec<PromptContentPart> = {
            let mut result = Vec::with_capacity(parts.len() + 1);
            let mut prefixed = false;
            for part in &parts {
                match part {
                    PromptContentPart::Text { text } => {
                        if !prefixed {
                            result.push(PromptContentPart::Text {
                                text: format!("Captain steer:\n{text}"),
                            });
                            prefixed = true;
                        } else {
                            result.push(PromptContentPart::Text { text: text.clone() });
                        }
                    }
                    PromptContentPart::Image { .. } => {
                        if !prefixed {
                            result.push(PromptContentPart::Text {
                                text: "Captain steer:\n".to_owned(),
                            });
                            prefixed = true;
                        }
                        result.push(part.clone());
                    }
                }
            }
            if !prefixed {
                result.push(PromptContentPart::Text {
                    text: "Captain steer:".to_owned(),
                });
            }
            result
        };
        let mut current_parts: Option<Vec<PromptContentPart>> = Some(initial_parts);
        let mut enforce_submit_attempts = 0u32;

        loop {
            let prompt_parts = current_parts.take().unwrap_or_default();
            let stop_reason = match self
                .prompt_agent(&session_id, Role::Mate, prompt_parts)
                .await
            {
                Ok(stop_reason) => stop_reason,
                Err(error) => {
                    Self::log_error("prompt_mate_steer", &error);
                    return;
                }
            };

            let pending_guidance = {
                let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                let Some(session) = sessions.get_mut(&session_id) else {
                    return;
                };
                Self::take_pending_mate_guidance(session)
            };
            if let Some(message) = pending_guidance {
                if let Err(error) = self.persist_session(&session_id).await {
                    Self::log_error("persist_pending_mate_guidance", &error);
                }
                current_parts = Some(vec![PromptContentPart::Text { text: message }]);
                continue;
            }

            match stop_reason {
                // r[task.completion.enforce-submit]
                ship_core::StopReason::EndTurn => {
                    let already_submitted = {
                        let sessions = self.sessions.lock().expect("sessions mutex poisoned");
                        sessions
                            .get(&session_id)
                            .map(|s| {
                                // No current task means it was archived (accepted/cancelled).
                                let Some(task) = s.current_task.as_ref() else {
                                    return true;
                                };
                                task.record.status == TaskStatus::ReviewPending
                                    || task.record.status.is_terminal()
                            })
                            .unwrap_or(false)
                    };

                    if already_submitted {
                        // mate_submit was called or task already in terminal state; captain already notified
                        break;
                    }

                    enforce_submit_attempts += 1;
                    if enforce_submit_attempts >= 2 {
                        let preamble = "The mate stopped repeatedly without submitting. \
                            Here is a reconstructed summary of recent work:";
                        if let Err(e) = self.force_mate_submit(&session_id, preamble).await {
                            Self::log_error("force_mate_submit", &e);
                        }
                        break;
                    }

                    current_parts = Some(vec![PromptContentPart::Text {
                        text: "<system-notification>You stopped without submitting your work. \
                            Call mate_submit with a summary of what you accomplished.\
                            </system-notification>"
                            .to_owned(),
                    }]);
                }
                ship_core::StopReason::ContextExhausted => {
                    {
                        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
                        if let Some(session) = sessions.get_mut(&session_id) {
                            set_agent_state(session, Role::Mate, AgentState::ContextExhausted);
                        }
                    }
                    let preamble = "The mate ran out of context without submitting. \
                        Here is a reconstructed summary of recent work:";
                    if let Err(e) = self.force_mate_submit(&session_id, preamble).await {
                        Self::log_error("force_mate_submit_context_exhausted", &e);
                    }
                    break;
                }
                other => {
                    if let Err(error) = self.handle_mate_stop_reason(&session_id, other).await {
                        Self::log_error("handle_mate_stop_reason_steer", &error);
                    }
                    break;
                }
            }
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
        let user_avatar_url = self
            .user_avatar_url
            .lock()
            .expect("user_avatar_url mutex poisoned")
            .clone();
        let sessions = self.sessions.lock().expect("sessions mutex poisoned");
        sessions
            .get(&id)
            .map(|s| Self::to_session_detail(s, user_avatar_url.clone()))
            .unwrap_or_else(|| Self::fallback_session_detail(id, user_avatar_url))
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
            created_at: chrono::Utc::now().to_rfc3339(),
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
                effort_config_id: None,
                effort_value_id: None,
                available_effort_values: Vec::new(),
            },
            mate: AgentSnapshot {
                role: Role::Mate,
                kind: req.mate_kind,
                state: AgentState::Idle,
                context_remaining_percent: None,
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
            captain_block_count: 0,
            mate_block_count: 0,
            pending_permissions: HashMap::new(),
            pending_edits: HashMap::new(),
            pending_steer: None,
            pending_human_review: None,
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

    async fn steer(&self, session: SessionId, parts: Vec<PromptContentPart>) {
        let parts = self.expand_file_mentions(&session, parts).await;
        if let Err(error) = self.dispatch_steer_to_mate(&session, parts).await {
            Self::log_error("steer", &error);
        }
    }

    // r[acp.prompt]
    // r[ui.composer.image-attach]
    async fn prompt_captain(&self, session: SessionId, parts: Vec<PromptContentPart>) {
        let parts = self.expand_file_mentions(&session, parts).await;
        if let Err(error) = self
            .append_human_message(&session, Role::Captain, &parts)
            .await
        {
            Self::log_error("prompt_captain_append_human_message", &error);
            return;
        }

        let this = self.clone();
        tokio::spawn(async move {
            if let Err(error) = this.interrupt_captain_with_parts(&session, parts).await {
                Self::log_error("prompt_captain", &error);
            }
        });
    }

    async fn accept(&self, session: SessionId) {
        if let Err(error) = self.accept_task(&session, None).await {
            Self::log_error("accept", &error);
        }
    }

    // r[proto.reply-to-human]
    async fn reply_to_human(&self, session: SessionId, message: String) {
        let tx = self
            .pending_mcp_ops
            .lock()
            .expect("pending_mcp_ops mutex poisoned")
            .get_mut(&session)
            .and_then(|ops| ops.human_reply.take());

        if let Some(tx) = tx {
            let _ = tx.send(message);
        } else {
            Self::log_error("reply_to_human", "no pending human reply for session");
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

    async fn retry_agent(&self, session: SessionId, role: Role) {
        match role {
            Role::Mate => {
                // Restart the mate process and re-dispatch to the active task if one exists.
                if let Err(error) = self.restart_mate(&session).await {
                    Self::log_error("retry_agent restart_mate", &error);
                    return;
                }

                let task_description = {
                    let sessions = self.sessions.lock().expect("sessions mutex poisoned");
                    sessions.get(&session).and_then(|s| {
                        s.current_task.as_ref().and_then(|t| {
                            if !t.record.status.is_terminal() {
                                Some(t.record.description.clone())
                            } else {
                                None
                            }
                        })
                    })
                };

                if let Some(description) = task_description {
                    let this = self.clone();
                    tokio::spawn(async move {
                        let parts = vec![PromptContentPart::Text {
                            text: Self::mate_task_preamble(&description),
                        }];
                        if let Err(error) = this.dispatch_steer_to_mate(&session, parts).await {
                            Self::log_error("retry_agent dispatch_steer_to_mate", &error);
                        }
                    });
                }
            }
            Role::Captain => {
                // Captain retry: just re-run the bootstrap prompt.
                let this = self.clone();
                tokio::spawn(async move {
                    if let Err(error) = this
                        .prompt_agent_text(
                            &session,
                            Role::Captain,
                            Self::captain_bootstrap_prompt(),
                        )
                        .await
                    {
                        Self::log_error("retry_agent captain", &error);
                    }
                });
            }
        }
    }

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

    // r[proto.set-agent-effort]
    async fn set_agent_effort(
        &self,
        session: SessionId,
        role: Role,
        config_id: String,
        value_id: String,
    ) -> SetAgentEffortResponse {
        let agent_driver = self.agent_driver.clone();
        let sessions = self.sessions.clone();

        let handle = {
            let sessions = sessions.lock().expect("sessions mutex poisoned");
            let Some(session_state) = sessions.get(&session) else {
                return SetAgentEffortResponse::SessionNotFound;
            };
            match role {
                Role::Captain => session_state.captain_handle.clone(),
                Role::Mate => session_state.mate_handle.clone(),
            }
        };

        let Some(handle) = handle else {
            return SetAgentEffortResponse::AgentNotSpawned;
        };

        match agent_driver
            .set_effort(&handle, &config_id, &value_id)
            .await
        {
            Ok(()) => {
                let mut sessions = sessions.lock().expect("sessions mutex poisoned");
                if let Some(session_state) = sessions.get_mut(&session) {
                    let available_effort_values = match role {
                        Role::Captain => session_state.captain.available_effort_values.clone(),
                        Role::Mate => session_state.mate.available_effort_values.clone(),
                    };
                    apply_event(
                        session_state,
                        ship_types::SessionEvent::AgentEffortChanged {
                            role,
                            effort_config_id: Some(config_id),
                            effort_value_id: Some(value_id),
                            available_effort_values,
                        },
                    );
                }
                SetAgentEffortResponse::Ok
            }
            Err(error) => SetAgentEffortResponse::Failed {
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
    // r[ui.composer.file-mention]
    async fn list_worktree_files(&self, session: SessionId) -> Vec<String> {
        match self.list_worktree_files_impl(&session).await {
            Ok(files) => files,
            Err(error) => {
                Self::log_error("list_worktree_files", &error);
                vec![]
            }
        }
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
                diffs: vec![],
            },
            Err(text) => McpToolCallResponse {
                text,
                is_error: true,
                diffs: vec![],
            },
        }
    }
}

impl CaptainMcp for CaptainMcpSessionService {
    // r[captain.tool.assign]
    async fn captain_assign(
        &self,
        title: String,
        description: String,
        keep: bool,
    ) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_assign(&self.session_id, title, description, keep)
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

    // r[captain.tool.read-only]
    async fn captain_read_file(
        &self,
        path: String,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_read_file(&self.session_id, path, offset, limit)
                .await,
        )
    }

    // r[captain.tool.read-only]
    async fn captain_search_files(&self, args: String) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_search_files(&self.session_id, args)
                .await,
        )
    }

    // r[captain.tool.read-only]
    async fn captain_list_files(&self, args: String) -> McpToolCallResponse {
        Self::response(
            self.ship
                .captain_tool_list_files(&self.session_id, args)
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
                diffs: vec![],
            },
            Err(text) => McpToolCallResponse {
                text,
                is_error: true,
                diffs: vec![],
            },
        }
    }
}

impl MateMcp for MateMcpSessionService {
    // r[mate.tool.run-command]
    async fn run_command(&self, command: String, cwd: Option<String>) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_run_command(&self.session_id, command, cwd)
                .await,
        )
    }

    // r[mate.tool.read-file]
    async fn read_file(
        &self,
        path: String,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_read_file(&self.session_id, path, offset, limit)
                .await,
        )
    }

    // r[mate.tool.write-file]
    async fn write_file(&self, path: String, content: String) -> McpToolCallResponse {
        self.ship
            .mate_tool_write_file(&self.session_id, path, content)
            .await
    }

    // r[mate.tool.edit-prepare]
    async fn edit_prepare(
        &self,
        path: String,
        old_string: String,
        new_string: String,
        replace_all: Option<bool>,
    ) -> McpToolCallResponse {
        self.ship
            .mate_tool_edit_prepare(&self.session_id, path, old_string, new_string, replace_all)
            .await
    }

    // r[mate.tool.edit-confirm]
    async fn edit_confirm(&self, edit_id: String) -> McpToolCallResponse {
        self.ship
            .mate_tool_edit_confirm(&self.session_id, edit_id)
            .await
    }

    // r[mate.tool.search-files]
    async fn search_files(&self, args: String) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_search_files(&self.session_id, args)
                .await,
        )
    }

    // r[mate.tool.list-files]
    async fn list_files(&self, args: String) -> McpToolCallResponse {
        Self::response(self.ship.mate_tool_list_files(&self.session_id, args).await)
    }

    // r[mate.tool.send-update]
    async fn mate_send_update(&self, message: String) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_send_update(&self.session_id, message)
                .await,
        )
    }

    // r[mate.tool.plan-create]
    async fn set_plan(&self, steps: Vec<String>) -> McpToolCallResponse {
        Self::response(self.ship.mate_tool_set_plan(&self.session_id, steps).await)
    }

    // r[mate.tool.plan-step-complete]
    async fn plan_step_complete(&self, step_index: u64, summary: String) -> McpToolCallResponse {
        let Ok(step_index) = usize::try_from(step_index) else {
            return McpToolCallResponse {
                text: "step_index is too large".to_owned(),
                is_error: true,
                diffs: vec![],
            };
        };
        Self::response(
            self.ship
                .mate_tool_plan_step_complete(&self.session_id, step_index, summary)
                .await,
        )
    }

    // r[mate.tool.cargo-check]
    async fn cargo_check(&self, args: Option<String>) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_cargo_check(&self.session_id, args)
                .await,
        )
    }

    // r[mate.tool.cargo-clippy]
    async fn cargo_clippy(&self, args: Option<String>) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_cargo_clippy(&self.session_id, args)
                .await,
        )
    }

    // r[mate.tool.cargo-test]
    async fn cargo_test(&self, args: Option<String>) -> McpToolCallResponse {
        Self::response(self.ship.mate_tool_cargo_test(&self.session_id, args).await)
    }

    // r[mate.tool.pnpm-install]
    async fn pnpm_install(&self, args: Option<String>) -> McpToolCallResponse {
        Self::response(
            self.ship
                .mate_tool_pnpm_install(&self.session_id, args)
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
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use ship_core::{ProjectRegistry, SessionStore};
    use ship_service::Ship;
    use ship_types::{
        AgentDiscovery, AgentKind, ContentBlock, CreateSessionRequest, CreateSessionResponse,
        CurrentTask, McpServerConfig, McpStdioServerConfig, PlanStep, PlanStepPriority,
        PlanStepStatus, ProjectName, PromptContentPart, SessionEvent, SessionEventEnvelope,
        SessionId, SessionStartupState, SubscribeMessage, TaskId, TaskRecord, TaskStatus,
    };
    use tokio::sync::{broadcast, mpsc};
    use tokio::time::timeout;

    use super::ShipImpl;

    static MATE_TOOL_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_mate_tool_tests() -> MutexGuard<'static, ()> {
        MATE_TOOL_TEST_LOCK
            .lock()
            .expect("mate tool test lock should not be poisoned")
    }

    struct TestRustfmtProgramGuard;

    impl TestRustfmtProgramGuard {
        fn set(program: &str) -> Self {
            *super::TEST_RUSTFMT_PROGRAM
                .lock()
                .expect("test rustfmt program mutex poisoned") = Some(OsString::from(program));
            Self
        }
    }

    impl Drop for TestRustfmtProgramGuard {
        fn drop(&mut self) {
            *super::TEST_RUSTFMT_PROGRAM
                .lock()
                .expect("test rustfmt program mutex poisoned") = None;
        }
    }

    struct TestRgProgramGuard;

    impl TestRgProgramGuard {
        fn set(program: &str) -> Self {
            *super::TEST_RG_PROGRAM
                .lock()
                .expect("test rg program mutex poisoned") = Some(OsString::from(program));
            Self
        }
    }

    impl Drop for TestRgProgramGuard {
        fn drop(&mut self) {
            *super::TEST_RG_PROGRAM
                .lock()
                .expect("test rg program mutex poisoned") = None;
        }
    }

    struct TestFdProgramGuard;

    impl TestFdProgramGuard {
        fn set(program: &str) -> Self {
            *super::TEST_FD_PROGRAM
                .lock()
                .expect("test fd program mutex poisoned") = Some(OsString::from(program));
            Self
        }
    }

    impl Drop for TestFdProgramGuard {
        fn drop(&mut self) {
            *super::TEST_FD_PROGRAM
                .lock()
                .expect("test fd program mutex poisoned") = None;
        }
    }

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
                    title: "Investigate workflow".to_owned(),
                    description: "Investigate workflow".to_owned(),
                    status: TaskStatus::Assigned,
                },
                mate_plan: Some(vec![PlanStep {
                    description: "Test step".to_owned(),
                    priority: PlanStepPriority::Medium,
                    status: PlanStepStatus::Pending,
                }]),
                pending_mate_guidance: None,
                content_history: Vec::new(),
                event_log: Vec::new(),
            });
        }

        (dir, ship, session_id)
    }

    fn init_git_repo(path: &std::path::Path) {
        let status = std::process::Command::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .arg(path)
            .status()
            .expect("git init should run");
        assert!(status.success(), "git init should succeed");

        for (key, value) in [
            ("user.name", "Ship Tests"),
            ("user.email", "ship-tests@example.com"),
        ] {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(["config", key, value])
                .status()
                .expect("git config should run");
            assert!(status.success(), "git config should succeed");
        }
    }

    fn parse_edit_id(response: &str) -> String {
        // The edit_prepare response text ends with the edit_id after ": "
        response
            .rsplit(": ")
            .next()
            .expect("edit_id should be at end of response text")
            .trim()
            .to_owned()
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
            timestamp: "2026-01-01T00:00:00Z".to_owned(),
            event: SessionEvent::TaskStarted {
                task_id: task_id.clone(),
                title: "Replay task".to_owned(),
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
                timestamp: "2026-01-01T00:00:00Z".to_owned(),
                event: SessionEvent::TaskStarted {
                    task_id: task_id.clone(),
                    title: "Replay task".to_owned(),
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
                timestamp: "2026-01-01T00:00:00Z".to_owned(),
                event: SessionEvent::TaskStarted {
                    task_id: live_task_id.clone(),
                    title: "Live task".to_owned(),
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
                timestamp: "2026-01-01T00:00:00Z".to_owned(),
                event: SessionEvent::TaskStarted {
                    task_id: live_task_id,
                    title: "Live task".to_owned(),
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

        ship.dispatch_steer_to_mate(
            &session_id,
            vec![PromptContentPart::Text {
                text: "Send the approved steer".to_owned(),
            }],
        )
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

    // r[verify mate.tool.plan-create]
    #[tokio::test]
    async fn mate_tools_require_plan_before_substantive_work() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) = create_session_for_workflow_test("mate-plan-gate").await;
        let project_root = dir.join("project");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
            // Clear the plan that the test helper sets up
            session.current_task.as_mut().unwrap().mate_plan = None;
        }

        let run_err = ship
            .mate_tool_run_command(&session_id, "echo hello".to_owned(), None)
            .await
            .expect_err("run_command without plan should fail");
        assert!(
            run_err.contains("set_plan"),
            "error should mention set_plan: {run_err}"
        );

        let write_err = ship
            .mate_tool_write_file(&session_id, "test.txt".to_owned(), "content".to_owned())
            .await;
        assert!(write_err.is_error, "write_file without plan should fail");
        assert!(
            write_err.text.contains("set_plan"),
            "error should mention set_plan: {}",
            write_err.text
        );

        let edit_err = ship
            .mate_tool_edit_prepare(
                &session_id,
                "test.txt".to_owned(),
                "old".to_owned(),
                "new".to_owned(),
                None,
            )
            .await;
        assert!(edit_err.is_error, "edit_prepare without plan should fail");
        assert!(
            edit_err.text.contains("set_plan"),
            "error should mention set_plan: {}",
            edit_err.text
        );

        let confirm_err = ship
            .mate_tool_edit_confirm(&session_id, "edit-0".to_owned())
            .await;
        assert!(
            confirm_err.is_error,
            "edit_confirm without plan should fail"
        );
        assert!(
            confirm_err.text.contains("set_plan"),
            "error should mention set_plan: {}",
            confirm_err.text
        );
    }

    // r[verify mate.tool.plan-create]
    // r[verify mate.tool.plan-step-complete]
    #[tokio::test]
    async fn mate_plan_tools_persist_plan_commit_worktree_and_notify_captain() {
        let (dir, ship, session_id) = create_session_for_workflow_test("mate-plan-tools").await;
        let project_root = dir.join("project");
        init_git_repo(&project_root);

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
            // Clear the plan the test helper pre-sets so set_plan takes the first-call path.
            session.current_task.as_mut().unwrap().mate_plan = None;
        }

        // set_plan stores the plan and notifies the captain non-blocking on first call.
        ship.mate_tool_set_plan(
            &session_id,
            vec!["Set up types".to_owned(), "Implement handler".to_owned()],
        )
        .await
        .expect("set_plan should succeed");

        {
            let sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get(&session_id).expect("session should exist");
            let task = session.current_task.as_ref().expect("task should exist");
            let plan = task.mate_plan.as_ref().expect("plan should be persisted");
            assert_eq!(plan.len(), 2);
            assert!(task.content_history.iter().any(|entry| matches!(
                &entry.block,
                ContentBlock::Text { text, .. } if text.contains("<system-notification>") && text.contains("The mate has set their plan.")
            )));
        }

        // Mate writes step 1's changes, then immediately calls plan_step_complete.
        std::fs::write(project_root.join("notes.txt"), "first draft\n")
            .expect("test file should be written");
        ship.mate_tool_plan_step_complete(&session_id, 0, "added initial notes".to_owned())
            .await
            .expect("plan_step_complete should succeed");

        let step_head = std::process::Command::new("git")
            .arg("-C")
            .arg(&project_root)
            .args(["rev-list", "--count", "HEAD"])
            .output()
            .expect("git rev-list should run");
        assert_eq!(String::from_utf8_lossy(&step_head.stdout).trim(), "1");

        {
            let sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get(&session_id).expect("session should exist");
            let task = session.current_task.as_ref().expect("task should exist");
            let plan = task
                .mate_plan
                .as_ref()
                .expect("plan should still be persisted");
            assert_eq!(plan[0].status, PlanStepStatus::Completed);
        }

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.read-file]
    #[tokio::test]
    async fn mate_read_file_formats_numbered_slices_and_errors() {
        let (dir, ship, session_id) = create_session_for_workflow_test("mate-read-file").await;
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join("src")).expect("src directory should be created");
        std::fs::write(
            project_root.join("src/lib.rs"),
            "first line\nsecond line\nthird line\n",
        )
        .expect("test file should be written");
        std::fs::write(project_root.join("empty.txt"), "").expect("empty file should be written");
        std::fs::write(project_root.join("binary.bin"), [0, 1, 2, 3])
            .expect("binary file should be written");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let full = ship
            .mate_tool_read_file(&session_id, "src/lib.rs".to_owned(), None, None)
            .await
            .expect("full read should succeed");
        assert_eq!(full, "1→first line\n2→second line\n3→third line");

        let slice = ship
            .mate_tool_read_file(&session_id, "src/lib.rs".to_owned(), Some(2), Some(1))
            .await
            .expect("sliced read should succeed");
        assert_eq!(
            slice,
            "2→second line\n(truncated — file has 3 lines, showing 2–2. Use offset/limit to read more.)"
        );

        let empty = ship
            .mate_tool_read_file(&session_id, "empty.txt".to_owned(), None, None)
            .await
            .expect("empty file should be readable");
        assert_eq!(empty, "File is empty.");

        let binary = ship
            .mate_tool_read_file(&session_id, "binary.bin".to_owned(), None, None)
            .await
            .expect_err("binary file should be rejected");
        assert_eq!(binary, "Binary file — cannot display.");

        let directory = ship
            .mate_tool_read_file(&session_id, "src".to_owned(), None, None)
            .await
            .expect_err("directory should be rejected");
        assert_eq!(directory, "Path is a directory, not a file.");

        let missing = ship
            .mate_tool_read_file(&session_id, "missing.rs".to_owned(), None, None)
            .await
            .expect_err("missing file should be rejected");
        assert_eq!(missing, "File not found: missing.rs");

        let escaped = ship
            .mate_tool_read_file(&session_id, "../Cargo.toml".to_owned(), None, None)
            .await
            .expect_err("path escape should be rejected");
        assert_eq!(escaped, "Path resolves outside the worktree.");

        let absolute = ship
            .mate_tool_read_file(
                &session_id,
                project_root.join("src/lib.rs").display().to_string(),
                None,
                None,
            )
            .await
            .expect_err("absolute path should be rejected");
        assert_eq!(absolute, "Absolute paths are not allowed.");

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.search-files]
    #[tokio::test]
    async fn mate_search_files_returns_matches_no_matches_and_truncates_output() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) = create_session_for_workflow_test("mate-search-files").await;
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join("src")).expect("src directory should be created");
        std::fs::write(
            project_root.join("src/lib.rs"),
            "fn alpha() {}\nfn beta() {}\n",
        )
        .expect("test file should be written");

        let large_output = (0..1_200)
            .map(|index| format!("alpha {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(project_root.join("many.txt"), format!("{large_output}\n"))
            .expect("large search corpus should be written");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let matches = ship
            .mate_tool_search_files(&session_id, "-n -F 'fn beta' src/lib.rs".to_owned())
            .await
            .expect("match search should succeed");
        assert_eq!(matches, "2:fn beta() {}");

        let no_matches = ship
            .mate_tool_search_files(&session_id, "-n -F 'does not exist' src/lib.rs".to_owned())
            .await
            .expect("no-match search should still succeed");
        assert_eq!(no_matches, "No matches found.");

        let truncated = ship
            .mate_tool_search_files(&session_id, "-n -F alpha many.txt".to_owned())
            .await
            .expect("large search should succeed");
        assert!(
            truncated.contains("(output truncated - 1200 lines total. Narrow your search.)"),
            "unexpected truncation output: {truncated}"
        );
        assert_eq!(truncated.lines().count(), 1001);

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.search-files]
    #[tokio::test]
    async fn mate_search_files_reports_missing_binary() {
        let _guard = lock_mate_tool_tests();
        let _rg_guard = TestRgProgramGuard::set("rg-does-not-exist-for-ship-tests");
        let (dir, ship, session_id) =
            create_session_for_workflow_test("mate-search-files-missing-rg").await;
        let project_root = dir.join("project");
        std::fs::write(project_root.join("file.txt"), "alpha\n").expect("test file should exist");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let error = ship
            .mate_tool_search_files(&session_id, "-n -F alpha file.txt".to_owned())
            .await
            .expect_err("missing rg should error");
        assert_eq!(error, "ripgrep (rg) is not installed.");

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.list-files]
    #[tokio::test]
    async fn mate_list_files_filters_results_and_reports_missing_binary() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) = create_session_for_workflow_test("mate-list-files").await;
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join("src/nested"))
            .expect("test directories should be created");
        std::fs::write(project_root.join("src/lib.rs"), "pub fn lib() {}\n")
            .expect("lib file should exist");
        std::fs::write(project_root.join("src/nested/main.rs"), "fn main() {}\n")
            .expect("main file should exist");
        std::fs::write(project_root.join("src/nested/readme.txt"), "notes\n")
            .expect("text file should exist");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let listed = ship
            .mate_tool_list_files(&session_id, ". src/ -e rs".to_owned())
            .await
            .expect("fd listing should succeed");
        assert!(listed.contains("src/lib.rs"), "unexpected output: {listed}");
        assert!(
            listed.contains("src/nested/main.rs"),
            "unexpected output: {listed}"
        );
        assert!(
            !listed.contains("readme.txt"),
            "extension filtering should exclude readme.txt: {listed}"
        );

        let _fd_guard = TestFdProgramGuard::set("fd-does-not-exist-for-ship-tests");
        let error = ship
            .mate_tool_list_files(&session_id, ". src/ -e rs".to_owned())
            .await
            .expect_err("missing fd should error");
        assert_eq!(error, "fd is not installed.");

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.run-command]
    #[tokio::test]
    async fn mate_run_command_executes_reports_failures_guards_cwd_and_truncates() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) = create_session_for_workflow_test("mate-run-command").await;
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join("nested"))
            .expect("nested directory should exist");
        std::fs::write(project_root.join("nested/value.txt"), "from nested\n")
            .expect("nested file should exist");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let simple = ship
            .mate_tool_run_command(&session_id, "echo hello".to_owned(), None)
            .await
            .expect("simple command should succeed");
        assert_eq!(simple, "hello\nexit code: 0");

        let failed = ship
            .mate_tool_run_command(&session_id, "false".to_owned(), None)
            .await
            .expect("failing command should still return output");
        assert_eq!(failed, "exit code: 1");

        let guarded = ship
            .mate_tool_run_command(&session_id, "git checkout .".to_owned(), None)
            .await
            .expect_err("dangerous command should be redirected");
        assert_eq!(
            guarded,
            "The command `git checkout .` has been blocked because it could affect the worktree \
in ways that are hard to undo. Use mate_ask_captain to explain what you need, \
and the captain will help you find the right approach."
        );

        let custom_cwd = ship
            .mate_tool_run_command(
                &session_id,
                "cat value.txt".to_owned(),
                Some("nested".to_owned()),
            )
            .await
            .expect("command should run in provided cwd");
        assert_eq!(custom_cwd, "from nested\nexit code: 0");

        let invalid_cwd = ship
            .mate_tool_run_command(&session_id, "pwd".to_owned(), Some("missing".to_owned()))
            .await
            .expect_err("missing cwd should fail");
        assert_eq!(invalid_cwd, "Directory not found: missing");

        let large_output = ship
            .mate_tool_run_command(
                &session_id,
                "i=1; while [ \"$i\" -le 1005 ]; do echo line-$i; i=$((i+1)); done".to_owned(),
                None,
            )
            .await
            .expect("large command output should succeed");
        assert!(
            large_output.contains("line-1"),
            "unexpected output: {large_output}"
        );
        assert!(
            large_output
                .contains("(output truncated - 1005 lines total, showing first 1000 lines.)"),
            "unexpected truncation output: {large_output}"
        );
        assert!(large_output.ends_with("exit code: 0"));
        assert_eq!(large_output.lines().count(), 1002);

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.write-file]
    #[tokio::test]
    async fn mate_write_file_writes_formats_and_creates_missing_parents() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) =
            create_session_for_workflow_test("mate-write-file-valid").await;
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join("src")).expect("src directory should be created");
        std::fs::write(project_root.join("src/blah.rs"), "pub fn helper() {}\n")
            .expect("module file should be written");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let result = ship
            .mate_tool_write_file(
                &session_id,
                "src/lib.rs".to_owned(),
                "mod blah;\npub fn main( ) -> u32 {1}\n".to_owned(),
            )
            .await;
        assert!(
            !result.is_error,
            "valid rust file should be written: {}",
            result.text
        );
        assert_eq!(result.text, "Wrote src/lib.rs (2 lines)");
        assert_eq!(
            std::fs::read_to_string(project_root.join("src/lib.rs"))
                .expect("written rust file should exist"),
            "mod blah;\npub fn main() -> u32 {\n    1\n}\n"
        );

        let nested = ship
            .mate_tool_write_file(
                &session_id,
                "notes/nested/file.txt".to_owned(),
                "alpha\nbeta\n".to_owned(),
            )
            .await;
        assert!(
            !nested.is_error,
            "nested non-rust write should succeed: {}",
            nested.text
        );
        assert_eq!(nested.text, "Wrote notes/nested/file.txt (2 lines)");
        assert_eq!(
            std::fs::read_to_string(project_root.join("notes/nested/file.txt"))
                .expect("nested file should exist"),
            "alpha\nbeta\n"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.write-file]
    #[tokio::test]
    async fn mate_write_file_rejects_bad_paths_and_rolls_back_invalid_rust() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) =
            create_session_for_workflow_test("mate-write-file-invalid").await;
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join("src")).expect("src directory should be created");
        std::fs::write(project_root.join("src/lib.rs"), "pub fn preserved() {}\n")
            .expect("existing rust file should be written");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let escaped = ship
            .mate_tool_write_file(&session_id, "../Cargo.toml".to_owned(), "nope".to_owned())
            .await;
        assert!(escaped.is_error, "path escape should be rejected");
        assert_eq!(escaped.text, "Path resolves outside the worktree.");

        let absolute = ship
            .mate_tool_write_file(
                &session_id,
                project_root.join("src/lib.rs").display().to_string(),
                "nope".to_owned(),
            )
            .await;
        assert!(absolute.is_error, "absolute path should be rejected");
        assert_eq!(absolute.text, "Absolute paths are not allowed.");

        let invalid = ship
            .mate_tool_write_file(
                &session_id,
                "src/lib.rs".to_owned(),
                "pub fn broken( {\n".to_owned(),
            )
            .await;
        assert!(invalid.is_error, "invalid rust file should be rejected");
        assert!(
            invalid.text.contains("Syntax error in src/lib.rs:"),
            "unexpected error: {}",
            invalid.text
        );
        assert_eq!(
            std::fs::read_to_string(project_root.join("src/lib.rs"))
                .expect("original file should be restored"),
            "pub fn preserved() {}\n"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.write-file]
    #[tokio::test]
    async fn mate_write_file_falls_back_when_rustfmt_is_unavailable() {
        let _guard = lock_mate_tool_tests();
        let _rustfmt_guard = TestRustfmtProgramGuard::set("rustfmt-does-not-exist-for-ship-tests");
        let (dir, ship, session_id) =
            create_session_for_workflow_test("mate-write-file-no-rustfmt").await;
        let project_root = dir.join("project");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let result = ship
            .mate_tool_write_file(
                &session_id,
                "src/lib.rs".to_owned(),
                "pub fn unformatted( ) -> u32 {1}\n".to_owned(),
            )
            .await;
        assert!(
            !result.is_error,
            "write should succeed without rustfmt: {}",
            result.text
        );
        assert_eq!(result.text, "Wrote src/lib.rs (1 lines)");
        assert_eq!(
            std::fs::read_to_string(project_root.join("src/lib.rs"))
                .expect("file should still be written"),
            "pub fn unformatted( ) -> u32 {1}\n"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.edit-prepare]
    // r[verify mate.tool.edit-confirm]
    #[tokio::test]
    async fn mate_edit_prepare_and_confirm_apply_valid_rust_edit() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) = create_session_for_workflow_test("mate-edit-confirm").await;
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join("src")).expect("src directory should be created");
        std::fs::write(
            project_root.join("src/lib.rs"),
            "pub fn greet() {\n    old_name();\n}\n",
        )
        .expect("source file should exist");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let prepared = ship
            .mate_tool_edit_prepare(
                &session_id,
                "src/lib.rs".to_owned(),
                "old_name();".to_owned(),
                "new_name( );".to_owned(),
                None,
            )
            .await;
        assert!(
            !prepared.is_error,
            "edit_prepare should succeed: {}",
            prepared.text
        );
        assert_eq!(prepared.diffs.len(), 1, "expected one diff");
        let diff = &prepared.diffs[0];
        assert_eq!(diff.path, "src/lib.rs");
        assert!(
            diff.old_text
                .as_deref()
                .unwrap_or("")
                .contains("old_name();"),
            "expected old content to contain old_name(): {:?}",
            diff.old_text
        );
        assert!(
            diff.new_text.contains("new_name()"),
            "expected new content to contain new_name(): {}",
            diff.new_text
        );
        let edit_id = parse_edit_id(&prepared.text);

        let confirmed = ship
            .mate_tool_edit_confirm(&session_id, edit_id.clone())
            .await;
        assert!(
            !confirmed.is_error,
            "edit_confirm should succeed: {}",
            confirmed.text
        );
        assert_eq!(confirmed.text, format!("Applied {edit_id} to src/lib.rs"));
        assert_eq!(
            std::fs::read_to_string(project_root.join("src/lib.rs"))
                .expect("edited file should exist"),
            "pub fn greet() {\n    new_name();\n}\n"
        );

        let sessions = ship.sessions.lock().expect("sessions mutex poisoned");
        let session = sessions.get(&session_id).expect("session should exist");
        assert!(
            session.pending_edits.is_empty(),
            "confirmed edit should clear pending edits"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.edit-prepare]
    #[tokio::test]
    async fn mate_edit_prepare_rejects_missing_and_ambiguous_matches() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) =
            create_session_for_workflow_test("mate-edit-prepare-errors").await;
        let project_root = dir.join("project");
        std::fs::write(project_root.join("notes.txt"), "alpha\nbeta\nalpha\n")
            .expect("test file should exist");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let missing = ship
            .mate_tool_edit_prepare(
                &session_id,
                "notes.txt".to_owned(),
                "gamma".to_owned(),
                "delta".to_owned(),
                None,
            )
            .await;
        assert!(missing.is_error, "missing old_string should fail");
        assert_eq!(missing.text, "old_string not found in notes.txt.");

        let ambiguous = ship
            .mate_tool_edit_prepare(
                &session_id,
                "notes.txt".to_owned(),
                "alpha".to_owned(),
                "delta".to_owned(),
                None,
            )
            .await;
        assert!(ambiguous.is_error, "ambiguous old_string should fail");
        assert_eq!(
            ambiguous.text,
            "old_string matches 2 locations in notes.txt. Provide more surrounding context to make the match unique."
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.edit-prepare]
    // r[verify mate.tool.edit-confirm]
    #[tokio::test]
    async fn mate_edit_multiple_edits_same_file_confirmed_in_sequence() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) =
            create_session_for_workflow_test("mate-edit-prepare-replace-all").await;
        let project_root = dir.join("project");
        std::fs::write(project_root.join("notes.txt"), "foo\nmiddle\nfoo\n")
            .expect("test file should exist");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        // Prepare two edits for the same file: both coexist in pending_edits.
        let first = ship
            .mate_tool_edit_prepare(
                &session_id,
                "notes.txt".to_owned(),
                "middle".to_owned(),
                "center".to_owned(),
                None,
            )
            .await;
        assert!(
            !first.is_error,
            "first prepare should succeed: {}",
            first.text
        );
        let first_id = parse_edit_id(&first.text);

        let second = ship
            .mate_tool_edit_prepare(
                &session_id,
                "notes.txt".to_owned(),
                "foo".to_owned(),
                "bar".to_owned(),
                Some(true),
            )
            .await;
        assert!(
            !second.is_error,
            "replace_all prepare should succeed: {}",
            second.text
        );
        assert_eq!(second.diffs.len(), 1, "expected one diff");
        assert!(
            second.diffs[0]
                .old_text
                .as_deref()
                .unwrap_or("")
                .contains("foo"),
            "unexpected old content: {:?}",
            second.diffs[0].old_text
        );
        assert_eq!(
            second.diffs[0].new_text.matches("bar").count(),
            2,
            "unexpected new content: {}",
            second.diffs[0].new_text
        );
        let second_id = parse_edit_id(&second.text);

        // Confirm first edit (middle → center). File becomes foo\ncenter\nfoo\n.
        let first_confirm = ship.mate_tool_edit_confirm(&session_id, first_id).await;
        assert!(
            !first_confirm.is_error,
            "first confirm should succeed: {}",
            first_confirm.text
        );

        // Confirm second edit (replace_all foo → bar). File changed since prepare
        // but old_string "foo" is still uniquely re-applicable → re-apply succeeds.
        let second_confirm = ship.mate_tool_edit_confirm(&session_id, second_id).await;
        assert!(
            !second_confirm.is_error,
            "replace_all confirm should succeed after re-apply: {}",
            second_confirm.text
        );
        assert_eq!(
            std::fs::read_to_string(project_root.join("notes.txt"))
                .expect("edited file should exist"),
            "bar\ncenter\nbar\n"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.edit-confirm]
    #[tokio::test]
    async fn mate_edit_confirm_rejects_stale_and_unknown_edits() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) =
            create_session_for_workflow_test("mate-edit-confirm-stale").await;
        let project_root = dir.join("project");
        std::fs::write(project_root.join("notes.txt"), "alpha\nbeta\n")
            .expect("test file should exist");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        let prepared = ship
            .mate_tool_edit_prepare(
                &session_id,
                "notes.txt".to_owned(),
                "beta".to_owned(),
                "gamma".to_owned(),
                None,
            )
            .await;
        assert!(
            !prepared.is_error,
            "prepare should succeed: {}",
            prepared.text
        );
        let edit_id = parse_edit_id(&prepared.text);

        // Overwrite the file so "beta" (the old_string) is gone.
        std::fs::write(project_root.join("notes.txt"), "alpha\nchanged\n")
            .expect("file mutation should succeed");

        // Re-apply is attempted but old_string "beta" is no longer in the file.
        let stale = ship.mate_tool_edit_confirm(&session_id, edit_id).await;
        assert!(
            stale.is_error,
            "stale edit should fail when old_string is gone"
        );
        assert!(
            stale.text.contains("re-apply failed"),
            "expected re-apply failure message, got: {}",
            stale.text
        );

        let unknown = ship
            .mate_tool_edit_confirm(&session_id, "edit-does-not-exist".to_owned())
            .await;
        assert!(unknown.is_error, "unknown edit id should fail");
        assert_eq!(
            unknown.text,
            "edit_id not found. It may have expired or been superseded."
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify mate.tool.edit-prepare]
    #[tokio::test]
    async fn mate_edit_prepare_rejects_invalid_rust_and_leaves_file_intact() {
        let _guard = lock_mate_tool_tests();
        let (dir, ship, session_id) =
            create_session_for_workflow_test("mate-edit-prepare-invalid-rust").await;
        let project_root = dir.join("project");
        std::fs::create_dir_all(project_root.join("src")).expect("src directory should be created");
        std::fs::write(project_root.join("src/lib.rs"), "pub fn intact() {}\n")
            .expect("source file should exist");

        {
            let mut sessions = ship.sessions.lock().expect("sessions mutex poisoned");
            let session = sessions.get_mut(&session_id).expect("session should exist");
            session.worktree_path = Some(project_root.clone());
        }

        // edit_prepare validates Rust syntax speculatively; broken edits are
        // rejected before the agent is asked to confirm.
        let err = ship
            .mate_tool_edit_prepare(
                &session_id,
                "src/lib.rs".to_owned(),
                "pub fn intact() {}\n".to_owned(),
                "pub fn broken( {\n".to_owned(),
                None,
            )
            .await;
        assert!(err.is_error, "prepare should fail for invalid Rust");
        assert!(
            err.text
                .contains("Edit would produce invalid Rust in src/lib.rs"),
            "unexpected error: {}",
            err.text
        );
        // Original file must be intact after the rejected prepare.
        assert_eq!(
            std::fs::read_to_string(project_root.join("src/lib.rs"))
                .expect("original file should be intact"),
            "pub fn intact() {}\n"
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
