use std::sync::Arc;

use facet::Facet;
use facet_json::RawJson;
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl InitializeRequest {
    pub fn new(protocol_version: ProtocolVersion) -> Self {
        Self {
            protocol_version,
            client_capabilities: ClientCapabilities::default(),
            client_info: None,
            meta: None,
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl InitializeResponse {
    pub fn new(protocol_version: ProtocolVersion) -> Self {
        Self {
            protocol_version,
            agent_capabilities: AgentCapabilities::default(),
            auth_methods: vec![],
            agent_info: None,
            meta: None,
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl Implementation {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            version: version.into(),
            meta: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

// ── Capabilities ───────────────────────────────────────────────────

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[facet(default)]
    pub load_session: bool,
    #[facet(default)]
    pub prompt_capabilities: PromptCapabilities,
    #[facet(default)]
    pub mcp_capabilities: McpCapabilities,
    #[facet(default)]
    pub session_capabilities: SessionCapabilities,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PromptCapabilities {
    #[facet(default)]
    pub image: bool,
    #[facet(default)]
    pub audio: bool,
    #[facet(default)]
    pub embedded_context: bool,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
pub struct SessionCapabilities {
    #[facet(default)]
    pub list: Option<SessionListCapabilities>,
    #[facet(default)]
    pub fork: Option<SessionForkCapabilities>,
    #[facet(default)]
    pub resume: Option<SessionResumeCapabilities>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
pub struct SessionListCapabilities {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
pub struct SessionForkCapabilities {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
pub struct SessionResumeCapabilities {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct McpCapabilities {
    #[facet(default)]
    pub http: bool,
    #[facet(default)]
    pub sse: bool,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Default, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[facet(default)]
    pub fs: FileSystemCapability,
    #[facet(default)]
    pub terminal: bool,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Default, Facet)]
#[facet(rename_all = "camelCase")]
pub struct FileSystemCapability {
    #[facet(default)]
    pub read_text_file: bool,
    #[facet(default)]
    pub write_text_file: bool,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AuthenticateRequest {
    pub method_id: AuthMethodId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AuthenticateResponse {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

// ── Sessions ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct NewSessionRequest {
    pub cwd: String,
    #[facet(default)]
    pub mcp_servers: Vec<McpServer>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl NewSessionRequest {
    pub fn new(cwd: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            mcp_servers: vec![],
            meta: None,
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl NewSessionResponse {
    pub fn new(session_id: impl Into<SessionId>) -> Self {
        Self {
            session_id: session_id.into(),
            modes: None,
            models: None,
            config_options: None,
            meta: None,
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct LoadSessionRequest {
    #[facet(default)]
    pub mcp_servers: Vec<McpServer>,
    pub cwd: String,
    pub session_id: SessionId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct LoadSessionResponse {
    #[facet(default)]
    pub modes: Option<SessionModeState>,
    #[facet(default)]
    pub models: Option<SessionModelState>,
    #[facet(default)]
    pub config_options: Option<Vec<SessionConfigOption>>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

// ── Fork Session ───────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ForkSessionRequest {
    pub session_id: SessionId,
    pub cwd: String,
    #[facet(default)]
    pub mcp_servers: Vec<McpServer>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ForkSessionResponse {
    pub session_id: SessionId,
    #[facet(default)]
    pub modes: Option<SessionModeState>,
    #[facet(default)]
    pub models: Option<SessionModelState>,
    #[facet(default)]
    pub config_options: Option<Vec<SessionConfigOption>>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

// ── Resume Session ─────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ResumeSessionRequest {
    pub session_id: SessionId,
    pub cwd: String,
    #[facet(default)]
    pub mcp_servers: Vec<McpServer>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ResumeSessionResponse {
    #[facet(default)]
    pub modes: Option<SessionModeState>,
    #[facet(default)]
    pub models: Option<SessionModelState>,
    #[facet(default)]
    pub config_options: Option<Vec<SessionConfigOption>>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

// ── List Sessions ──────────────────────────────────────────────────

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ListSessionsRequest {
    #[facet(default)]
    pub cwd: Option<String>,
    #[facet(default)]
    pub cursor: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionInfo>,
    #[facet(default)]
    pub next_cursor: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub cwd: String,
    #[facet(default)]
    pub title: Option<String>,
    #[facet(default)]
    pub updated_at: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

// ── Prompts ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PromptRequest {
    pub session_id: SessionId,
    pub prompt: Vec<ContentBlock>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl PromptRequest {
    pub fn new(session_id: impl Into<SessionId>, prompt: Vec<ContentBlock>) -> Self {
        Self {
            session_id: session_id.into(),
            prompt,
            meta: None,
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PromptResponse {
    pub stop_reason: StopReason,
    #[facet(default)]
    pub usage: Option<Usage>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

/// Token usage information for a prompt turn.
#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Usage {
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[facet(default)]
    pub thought_tokens: Option<u64>,
    #[facet(default)]
    pub cached_read_tokens: Option<u64>,
    #[facet(default)]
    pub cached_write_tokens: Option<u64>,
}

/// Why the agent stopped.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    MaxTurnRequests,
    Refusal,
    Cancelled,
}

// ── Cancel ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct CancelNotification {
    pub session_id: SessionId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl CancelNotification {
    pub fn new(session_id: impl Into<SessionId>) -> Self {
        Self {
            session_id: session_id.into(),
            meta: None,
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
    pub current_mode_id: SessionModeId,
    pub available_modes: Vec<SessionMode>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionMode {
    pub id: SessionModeId,
    pub name: String,
    #[facet(default)]
    pub description: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionModeRequest {
    pub session_id: SessionId,
    pub mode_id: SessionModeId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionModeResponse {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

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
    pub current_model_id: ModelId,
    pub available_models: Vec<ModelInfo>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ModelInfo {
    pub model_id: ModelId,
    pub name: String,
    #[facet(default)]
    pub description: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionModelRequest {
    pub session_id: SessionId,
    pub model_id: ModelId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl SetSessionModelRequest {
    pub fn new(session_id: impl Into<SessionId>, model_id: impl Into<ModelId>) -> Self {
        Self {
            session_id: session_id.into(),
            model_id: model_id.into(),
            meta: None,
        }
    }
}

#[derive(Default, Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionModelResponse {
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct SessionConfigGroupId(pub Arc<str>);

impl SessionConfigGroupId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for SessionConfigGroupId {
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
    #[facet(default)]
    pub category: Option<SessionConfigOptionCategory>,
    pub kind: SessionConfigKind,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(tag = "type", rename_all = "snake_case")]
#[repr(u8)]
pub enum SessionConfigKind {
    Select(SessionConfigSelect),
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionConfigSelect {
    pub current_value: SessionConfigValueId,
    pub options: SessionConfigSelectOptions,
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
    pub value: SessionConfigValueId,
    pub name: String,
    #[facet(default)]
    pub description: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SessionConfigSelectGroup {
    pub group: SessionConfigGroupId,
    pub name: String,
    pub options: Vec<SessionConfigSelectOption>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum SessionConfigOptionCategory {
    Mode,
    Model,
    ThoughtLevel,
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionConfigOptionRequest {
    pub session_id: SessionId,
    pub config_id: SessionConfigId,
    pub value: SessionConfigValueId,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl SetSessionConfigOptionRequest {
    pub fn new(
        session_id: impl Into<SessionId>,
        config_id: impl Into<SessionConfigId>,
        value: impl Into<SessionConfigValueId>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            config_id: config_id.into(),
            value: value.into(),
            meta: None,
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct SetSessionConfigOptionResponse {
    pub config_options: Vec<SessionConfigOption>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

// ── MCP Servers ────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(tag = "type", rename_all = "snake_case")]
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl McpServerHttp {
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            headers: vec![],
            meta: None,
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl McpServerSse {
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            headers: vec![],
            meta: None,
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
    pub command: String,
    #[facet(default)]
    pub args: Vec<String>,
    #[facet(default)]
    pub env: Vec<EnvVariable>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl McpServerStdio {
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: vec![],
            env: vec![],
            meta: None,
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
#[facet(rename_all = "camelCase")]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl HttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            meta: None,
        }
    }
}

#[derive(Debug, Clone, Facet)]
#[facet(rename_all = "camelCase")]
pub struct EnvVariable {
    pub name: String,
    pub value: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl EnvVariable {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            meta: None,
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
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl UsageUpdate {
    pub fn new(used: u64, size: u64) -> Self {
        Self {
            used,
            size,
            cost: None,
            meta: None,
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
