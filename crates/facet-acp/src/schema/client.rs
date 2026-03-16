use std::sync::Arc;

use facet::Facet;
use facet_json::RawJson;

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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl SessionNotification {
    pub fn new(session_id: impl Into<SessionId>, update: SessionUpdate) -> Self {
        Self {
            session_id: session_id.into(),
            update,
            meta: None,
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
    SessionInfoUpdate(SessionInfoUpdate),
    UsageUpdate(UsageUpdate),
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct CurrentModeUpdate {
    pub current_mode_id: SessionModeId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl CurrentModeUpdate {
    pub fn new(current_mode_id: impl Into<SessionModeId>) -> Self {
        Self {
            current_mode_id: current_mode_id.into(),
            meta: None,
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ConfigOptionUpdate {
    pub config_options: Vec<SessionConfigOption>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl ConfigOptionUpdate {
    pub fn new(config_options: Vec<SessionConfigOption>) -> Self {
        Self {
            config_options,
            meta: None,
        }
    }
}

/// Update to session metadata.
#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionInfoUpdate {
    #[facet(default)]
    pub title: Option<String>,
    #[facet(default)]
    pub updated_at: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AvailableCommandsUpdate {
    pub available_commands: Vec<AvailableCommand>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AvailableCommand {
    pub name: String,
    pub description: String,
    #[facet(default)]
    pub input: Option<AvailableCommandInput>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl AvailableCommand {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input: None,
            meta: None,
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
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
    pub tool_call: ToolCallUpdate,
    #[facet(default)]
    pub options: Vec<PermissionOption>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: PermissionOptionId,
    pub name: String,
    pub kind: PermissionOptionKind,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl RequestPermissionResponse {
    pub fn new(outcome: RequestPermissionOutcome) -> Self {
        Self {
            outcome,
            meta: None,
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(tag = "outcome", rename_all = "snake_case")]
#[repr(u8)]
pub enum RequestPermissionOutcome {
    Cancelled {},
    Selected(SelectedPermissionOutcome),
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SelectedPermissionOutcome {
    pub option_id: PermissionOptionId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl SelectedPermissionOutcome {
    pub fn new(option_id: impl Into<PermissionOptionId>) -> Self {
        Self {
            option_id: option_id.into(),
            meta: None,
        }
    }
}

// ── File System ────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ReadTextFileRequest {
    pub session_id: SessionId,
    pub path: String,
    #[facet(default)]
    pub line: Option<u32>,
    #[facet(default)]
    pub limit: Option<u32>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ReadTextFileResponse {
    pub content: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct WriteTextFileRequest {
    pub session_id: SessionId,
    pub path: String,
    pub content: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct WriteTextFileResponse {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

// ── Terminals ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct CreateTerminalRequest {
    pub session_id: SessionId,
    pub command: String,
    #[facet(default)]
    pub args: Vec<String>,
    #[facet(default)]
    pub env: Vec<crate::EnvVariable>,
    #[facet(default)]
    pub cwd: Option<String>,
    #[facet(default)]
    pub output_byte_limit: Option<u64>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct CreateTerminalResponse {
    pub terminal_id: TerminalId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TerminalOutputRequest {
    pub session_id: SessionId,
    pub terminal_id: TerminalId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TerminalOutputResponse {
    pub output: String,
    pub truncated: bool,
    #[facet(default)]
    pub exit_status: Option<TerminalExitStatus>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct KillTerminalCommandRequest {
    pub session_id: SessionId,
    pub terminal_id: TerminalId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct KillTerminalCommandResponse {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ReleaseTerminalRequest {
    pub session_id: SessionId,
    pub terminal_id: TerminalId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ReleaseTerminalResponse {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct WaitForTerminalExitRequest {
    pub session_id: SessionId,
    pub terminal_id: TerminalId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct WaitForTerminalExitResponse {
    // In the reference this is #[serde(flatten)], but facet doesn't support flatten,
    // so we keep the nested struct.
    pub exit_status: TerminalExitStatus,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TerminalExitStatus {
    #[facet(default)]
    pub exit_code: Option<u32>,
    #[facet(default)]
    pub signal: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
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
