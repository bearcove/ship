use std::path::PathBuf;
use std::sync::Arc;

use facet::Facet;
use crate::{ContentBlock, SessionId, ProtocolVersion};

// ── Initialize ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct InitializeRequest {
    pub protocol_version: ProtocolVersion,
    #[facet(default)]
    pub client_capabilities: ClientCapabilities,
    #[facet(default)]
    pub client_info: Option<Implementation>,
}

impl InitializeRequest {
    pub fn new(protocol_version: ProtocolVersion) -> Self {
        Self {
            protocol_version,
            client_capabilities: ClientCapabilities::default(),
            client_info: None,
        }
    }

    pub fn client_capabilities(mut self, caps: ClientCapabilities) -> Self {
        self.client_capabilities = caps;
        self
    }

    pub fn client_info(mut self, info: Implementation) -> Self {
        self.client_info = Some(info);
        self
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub protocol_version: ProtocolVersion,
    #[facet(default)]
    pub agent_capabilities: AgentCapabilities,
    #[facet(default)]
    pub auth_methods: Vec<AuthMethod>,
    #[facet(default)]
    pub agent_info: Option<Implementation>,
}

impl InitializeResponse {
    pub fn new(protocol_version: ProtocolVersion) -> Self {
        Self {
            protocol_version,
            agent_capabilities: AgentCapabilities::default(),
            auth_methods: vec![],
            agent_info: None,
        }
    }
}

/// Metadata about an implementation.
#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Implementation {
    pub name: String,
    #[facet(default)]
    pub title: Option<String>,
    pub version: String,
}

impl Implementation {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            version: version.into(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

// ── Capabilities ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Facet)]
#[facet(rename_all = "camelCase", skip_all_unless_truthy)]
pub struct ClientCapabilities {
    #[facet(default)]
    pub fs: Option<FileSystemCapability>,
    #[facet(default)]
    pub terminal: Option<TerminalCapability>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct FileSystemCapability {
    #[facet(default)]
    pub read: Option<bool>,
    #[facet(default)]
    pub write: Option<bool>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TerminalCapability {
    #[facet(default)]
    pub create: Option<bool>,
}

#[derive(Debug, Clone, Default, Facet)]
#[facet(rename_all = "camelCase", skip_all_unless_truthy)]
pub struct AgentCapabilities {
    #[facet(default)]
    pub prompt: Option<PromptCapability>,
    #[facet(default)]
    pub session: Option<SessionCapability>,
    #[facet(default)]
    pub mcp: Option<McpCapability>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PromptCapability {
    #[facet(default)]
    pub image: Option<bool>,
    #[facet(default)]
    pub audio: Option<bool>,
    #[facet(default)]
    pub embedded_context: Option<bool>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionCapability {
    #[facet(default)]
    pub load: Option<bool>,
    #[facet(default)]
    pub resume: Option<bool>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct McpCapability {
    #[facet(default)]
    pub http: Option<bool>,
    #[facet(default)]
    pub sse: Option<bool>,
}

// ── Authentication ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct AuthMethodId(pub Arc<str>);

impl AuthMethodId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for AuthMethodId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AuthMethod {
    pub id: AuthMethodId,
    pub name: String,
    #[facet(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AuthenticateRequest {
    pub method_id: AuthMethodId,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AuthenticateResponse {}

// ── Sessions ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct NewSessionRequest {
    pub cwd: PathBuf,
    #[facet(default)]
    pub mcp_servers: Vec<McpServer>,
}

impl NewSessionRequest {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            mcp_servers: vec![],
        }
    }

    pub fn mcp_servers(mut self, servers: Vec<McpServer>) -> Self {
        self.mcp_servers = servers;
        self
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct NewSessionResponse {
    pub session_id: SessionId,
    #[facet(default)]
    pub modes: Option<SessionModeState>,
    #[facet(default)]
    pub models: Option<SessionModelState>,
    #[facet(default)]
    pub config_options: Option<Vec<SessionConfigOption>>,
}

impl NewSessionResponse {
    pub fn new(session_id: impl Into<SessionId>) -> Self {
        Self {
            session_id: session_id.into(),
            modes: None,
            models: None,
            config_options: None,
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct LoadSessionRequest {
    pub session_id: SessionId,
    pub cwd: PathBuf,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct LoadSessionResponse {}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ResumeSessionRequest {
    pub session_id: SessionId,
    pub cwd: PathBuf,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ResumeSessionResponse {}

// ── Prompts ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PromptRequest {
    pub session_id: SessionId,
    pub content: Vec<ContentBlock>,
    #[facet(default)]
    pub command: Option<String>,
}

impl PromptRequest {
    pub fn new(session_id: impl Into<SessionId>, content: Vec<ContentBlock>) -> Self {
        Self {
            session_id: session_id.into(),
            content,
            command: None,
        }
    }

    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PromptResponse {
    pub stop_reason: StopReason,
}

/// Why the agent stopped.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum StopReason {
    EndTurn,
    Cancelled,
    MaxTokens,
    MaxTurnRequests,
    Refusal,
}

// ── Cancel ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct CancelNotification {
    pub session_id: SessionId,
}

impl CancelNotification {
    pub fn new(session_id: impl Into<SessionId>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }
}

// ── Session Modes ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct SessionModeId(pub Arc<str>);

impl SessionModeId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for SessionModeId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionModeState {
    pub modes: Vec<SessionMode>,
    pub current_mode_id: SessionModeId,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionMode {
    pub id: SessionModeId,
    pub name: String,
    #[facet(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionModeRequest {
    pub session_id: SessionId,
    pub mode_id: SessionModeId,
}

#[derive(Default, Debug, Clone, Facet)]
pub struct SetSessionModeResponse {}

// ── Session Models ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct ModelId(pub Arc<str>);

impl ModelId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for ModelId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionModelState {
    pub models: Vec<SessionModel>,
    pub current_model_id: ModelId,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionModel {
    pub id: ModelId,
    pub name: String,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionModelRequest {
    pub session_id: SessionId,
    pub model_id: ModelId,
}

impl SetSessionModelRequest {
    pub fn new(session_id: impl Into<SessionId>, model_id: impl Into<ModelId>) -> Self {
        Self {
            session_id: session_id.into(),
            model_id: model_id.into(),
        }
    }
}

#[derive(Default, Debug, Clone, Facet)]
pub struct SetSessionModelResponse {}

// ── Session Config ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct SessionConfigId(pub Arc<str>);

impl SessionConfigId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for SessionConfigId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct SessionConfigValueId(pub Arc<str>);

impl SessionConfigValueId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for SessionConfigValueId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionConfigOption {
    pub id: SessionConfigId,
    pub name: String,
    #[facet(default)]
    pub description: Option<String>,
    pub kind: SessionConfigKind,
    #[facet(default)]
    pub category: Option<SessionConfigOptionCategory>,
}

#[derive(Debug, Clone, Facet)]
#[facet(tag = "type", rename_all = "snake_case")]
#[repr(u8)]
pub enum SessionConfigKind {
    Select(SessionConfigSelect),
    Boolean(SessionConfigBoolean),
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionConfigSelect {
    pub options: SessionConfigSelectOptions,
    pub current_value_id: SessionConfigValueId,
}

#[derive(Debug, Clone, Facet)]
#[facet(untagged)]
#[repr(u8)]
pub enum SessionConfigSelectOptions {
    Ungrouped(Vec<SessionConfigSelectOption>),
    Grouped(Vec<SessionConfigSelectGroup>),
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionConfigSelectOption {
    pub id: SessionConfigValueId,
    pub name: String,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionConfigSelectGroup {
    pub name: String,
    pub options: Vec<SessionConfigSelectOption>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionConfigBoolean {
    pub current_value: bool,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum SessionConfigOptionCategory {
    ThoughtLevel,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionConfigOptionRequest {
    pub session_id: SessionId,
    pub config_id: SessionConfigId,
    pub value_id: SessionConfigValueId,
}

impl SetSessionConfigOptionRequest {
    pub fn new(
        session_id: impl Into<SessionId>,
        config_id: impl Into<SessionConfigId>,
        value_id: impl Into<SessionConfigValueId>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            config_id: config_id.into(),
            value_id: value_id.into(),
        }
    }
}

#[derive(Default, Debug, Clone, Facet)]
pub struct SetSessionConfigOptionResponse {}

// ── MCP Servers ────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(tag = "transport", rename_all = "snake_case")]
#[repr(u8)]
pub enum McpServer {
    Http(McpServerHttp),
    Sse(McpServerSse),
    Stdio(McpServerStdio),
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct McpServerHttp {
    pub name: String,
    pub url: String,
    #[facet(default)]
    pub headers: Vec<HttpHeader>,
}

impl McpServerHttp {
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            headers: vec![],
        }
    }

    pub fn headers(mut self, headers: Vec<HttpHeader>) -> Self {
        self.headers = headers;
        self
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct McpServerSse {
    pub name: String,
    pub url: String,
    #[facet(default)]
    pub headers: Vec<HttpHeader>,
}

impl McpServerSse {
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            headers: vec![],
        }
    }

    pub fn headers(mut self, headers: Vec<HttpHeader>) -> Self {
        self.headers = headers;
        self
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct McpServerStdio {
    pub name: String,
    pub command: PathBuf,
    #[facet(default)]
    pub args: Vec<String>,
    #[facet(default)]
    pub env: Vec<EnvVariable>,
}

impl McpServerStdio {
    pub fn new(name: impl Into<String>, command: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: vec![],
            env: vec![],
        }
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn env(mut self, env: Vec<EnvVariable>) -> Self {
        self.env = env;
        self
    }
}

#[derive(Debug, Clone, Facet)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

impl HttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Facet)]
pub struct EnvVariable {
    pub name: String,
    pub value: String,
}

impl EnvVariable {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

// ── Usage ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct UsageUpdate {
    pub used: u64,
    pub size: u64,
    #[facet(default)]
    pub cost: Option<Cost>,
}

impl UsageUpdate {
    pub fn new(used: u64, size: u64) -> Self {
        Self {
            used,
            size,
            cost: None,
        }
    }

    pub fn cost(mut self, cost: Cost) -> Self {
        self.cost = Some(cost);
        self
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Cost {
    pub amount: f64,
    pub currency: String,
}

impl Cost {
    pub fn new(amount: f64, currency: impl Into<String>) -> Self {
        Self {
            amount,
            currency: currency.into(),
        }
    }
}
