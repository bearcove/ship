use std::path::PathBuf;
use std::sync::Arc;

use facet::Facet;

use crate::{
    ContentChunk, ExtNotification, ExtRequest, ExtResponse, InitializeRequest, Plan,
    SessionConfigOption, SessionId, SessionModeId, TerminalId, ToolCall, ToolCallUpdate, UsageUpdate,
};

// ── Session Notification ───────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionNotification {
    pub session_id: SessionId,
    pub update: SessionUpdate,
}

impl SessionNotification {
    pub fn new(session_id: impl Into<SessionId>, update: SessionUpdate) -> Self {
        Self {
            session_id: session_id.into(),
            update,
        }
    }
}

/// Different types of updates that can be sent during session processing.
#[derive(Debug, Clone, Facet)]
#[facet(tag = "sessionUpdate", rename_all = "snake_case")]
#[repr(u8)]
pub enum SessionUpdate {
    UserMessageChunk(ContentChunk),
    AgentMessageChunk(ContentChunk),
    AgentThoughtChunk(ContentChunk),
    ToolCall(ToolCall),
    ToolCallUpdate(ToolCallUpdate),
    Plan(Plan),
    AvailableCommandsUpdate(AvailableCommandsUpdate),
    CurrentModeUpdate(CurrentModeUpdate),
    ConfigOptionUpdate(ConfigOptionUpdate),
    UsageUpdate(UsageUpdate),
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct CurrentModeUpdate {
    pub current_mode_id: SessionModeId,
}

impl CurrentModeUpdate {
    pub fn new(current_mode_id: impl Into<SessionModeId>) -> Self {
        Self {
            current_mode_id: current_mode_id.into(),
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ConfigOptionUpdate {
    pub config_options: Vec<SessionConfigOption>,
}

impl ConfigOptionUpdate {
    pub fn new(config_options: Vec<SessionConfigOption>) -> Self {
        Self { config_options }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AvailableCommandsUpdate {
    pub available_commands: Vec<AvailableCommand>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AvailableCommand {
    pub name: String,
    pub description: String,
    #[facet(default)]
    pub input: Option<AvailableCommandInput>,
}

impl AvailableCommand {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input: None,
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(untagged, rename_all = "camelCase")]
#[repr(u8)]
pub enum AvailableCommandInput {
    Unstructured(UnstructuredCommandInput),
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct UnstructuredCommandInput {
    pub hint: String,
}

// ── Permission ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct PermissionOptionId(pub Arc<str>);

impl PermissionOptionId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for PermissionOptionId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl std::fmt::Display for PermissionOptionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct RequestPermissionRequest {
    pub session_id: SessionId,
    pub tool_call: ToolCall,
    #[facet(default)]
    pub options: Vec<PermissionOption>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: PermissionOptionId,
    pub name: String,
    pub kind: PermissionOptionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum PermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct RequestPermissionResponse {
    pub outcome: RequestPermissionOutcome,
}

impl RequestPermissionResponse {
    pub fn new(outcome: RequestPermissionOutcome) -> Self {
        Self { outcome }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(tag = "type", rename_all = "snake_case")]
#[repr(u8)]
pub enum RequestPermissionOutcome {
    Selected(SelectedPermissionOutcome),
    Cancelled {},
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SelectedPermissionOutcome {
    pub option_id: PermissionOptionId,
}

impl SelectedPermissionOutcome {
    pub fn new(option_id: impl Into<PermissionOptionId>) -> Self {
        Self {
            option_id: option_id.into(),
        }
    }
}

// ── File System ────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ReadTextFileRequest {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ReadTextFileResponse {
    pub content: String,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct WriteTextFileRequest {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Default, Debug, Clone, Facet)]
pub struct WriteTextFileResponse {}

// ── Terminals ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct CreateTerminalRequest {
    pub session_id: SessionId,
    pub command: String,
    #[facet(default)]
    pub args: Vec<String>,
    #[facet(default)]
    pub cwd: Option<PathBuf>,
    #[facet(default)]
    pub env: Option<Vec<crate::EnvVariable>>,
    #[facet(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct CreateTerminalResponse {
    pub terminal_id: TerminalId,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TerminalOutputRequest {
    pub terminal_id: TerminalId,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TerminalOutputResponse {
    pub output: String,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct KillTerminalCommandRequest {
    pub terminal_id: TerminalId,
}

#[derive(Default, Debug, Clone, Facet)]
pub struct KillTerminalCommandResponse {}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ReleaseTerminalRequest {
    pub terminal_id: TerminalId,
}

#[derive(Default, Debug, Clone, Facet)]
pub struct ReleaseTerminalResponse {}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct WaitForTerminalExitRequest {
    pub terminal_id: TerminalId,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct WaitForTerminalExitResponse {
    pub exit_status: TerminalExitStatus,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TerminalExitStatus {
    #[facet(default)]
    pub exit_code: Option<i32>,
    #[facet(default)]
    pub signal: Option<String>,
}

// ── Agent/Client Traits ────────────────────────────────────────────

/// The Agent trait — methods callable on the agent by the client.
#[async_trait::async_trait(?Send)]
pub trait Agent {
    async fn initialize(
        &self,
        args: InitializeRequest,
    ) -> crate::Result<crate::InitializeResponse>;

    async fn authenticate(
        &self,
        args: crate::AuthenticateRequest,
    ) -> crate::Result<crate::AuthenticateResponse>;

    async fn new_session(
        &self,
        args: crate::NewSessionRequest,
    ) -> crate::Result<crate::NewSessionResponse>;

    async fn load_session(
        &self,
        args: crate::LoadSessionRequest,
    ) -> crate::Result<crate::LoadSessionResponse>;

    async fn set_session_mode(
        &self,
        args: crate::SetSessionModeRequest,
    ) -> crate::Result<crate::SetSessionModeResponse>;

    async fn prompt(
        &self,
        args: crate::PromptRequest,
    ) -> crate::Result<crate::PromptResponse>;

    async fn cancel(&self, args: crate::CancelNotification) -> crate::Result<()>;

    async fn set_session_model(
        &self,
        args: crate::SetSessionModelRequest,
    ) -> crate::Result<crate::SetSessionModelResponse>;

    async fn resume_session(
        &self,
        args: crate::ResumeSessionRequest,
    ) -> crate::Result<crate::ResumeSessionResponse>;

    async fn set_session_config_option(
        &self,
        args: crate::SetSessionConfigOptionRequest,
    ) -> crate::Result<crate::SetSessionConfigOptionResponse>;

    async fn ext_method(&self, args: ExtRequest) -> crate::Result<ExtResponse>;

    async fn ext_notification(&self, args: ExtNotification) -> crate::Result<()>;
}

/// The Client trait — methods callable on the client by the agent.
#[async_trait::async_trait(?Send)]
pub trait Client {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> crate::Result<RequestPermissionResponse>;

    async fn session_notification(&self, args: SessionNotification) -> crate::Result<()>;

    async fn write_text_file(
        &self,
        args: WriteTextFileRequest,
    ) -> crate::Result<WriteTextFileResponse>;

    async fn read_text_file(
        &self,
        args: ReadTextFileRequest,
    ) -> crate::Result<ReadTextFileResponse>;

    async fn create_terminal(
        &self,
        args: CreateTerminalRequest,
    ) -> crate::Result<CreateTerminalResponse>;

    async fn terminal_output(
        &self,
        args: TerminalOutputRequest,
    ) -> crate::Result<TerminalOutputResponse>;

    async fn release_terminal(
        &self,
        args: ReleaseTerminalRequest,
    ) -> crate::Result<ReleaseTerminalResponse>;

    async fn wait_for_terminal_exit(
        &self,
        args: WaitForTerminalExitRequest,
    ) -> crate::Result<WaitForTerminalExitResponse>;

    async fn kill_terminal_command(
        &self,
        args: KillTerminalCommandRequest,
    ) -> crate::Result<KillTerminalCommandResponse>;

    async fn ext_method(&self, args: ExtRequest) -> crate::Result<ExtResponse>;

    async fn ext_notification(&self, args: ExtNotification) -> crate::Result<()>;
}
