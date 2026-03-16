pub mod client;
pub mod driver;
pub mod fakes;
pub mod launcher;
pub mod mcp;

use core::fmt;
use std::error::Error;
use std::path::PathBuf;
use std::pin::Pin;

use futures_core::Stream;
use ship_types::{
    AgentKind, EffortValue, McpServerConfig, Role, SessionEvent,
};

pub use driver::AcpAgentDriver;
pub use launcher::{
    AgentLauncher, BinaryPathProbe, SystemBinaryPathProbe, discover_agents, resolve_agent_launcher,
};
pub use mcp::{McpConfigError, resolve_mcp_servers};
pub use fakes::{FakeAgentDriver, FakePromptScript, SpawnRecord};

/// Opaque identifier for a running ACP agent process.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AcpSessionId(String);

impl AcpSessionId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentHandle {
    id: AcpSessionId,
}

impl AgentHandle {
    pub fn new(id: AcpSessionId) -> Self {
        Self { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    Cancelled,
    ContextExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptResponse {
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentError {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionConfig {
    pub worktree_path: PathBuf,
    pub mcp_servers: Vec<McpServerConfig>,
    /// ACP session ID from a previous run, used for session resume
    pub resume_session_id: Option<String>,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for AgentError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpawnInfo {
    pub handle: AgentHandle,
    pub model_id: Option<String>,
    pub available_models: Vec<String>,
    pub effort_config_id: Option<String>,
    pub effort_value_id: Option<String>,
    pub available_effort_values: Vec<EffortValue>,
    /// The ACP session ID, for persisting and later resuming
    pub acp_session_id: String,
    /// Whether the ACP session was resumed from a previous run
    pub was_resumed: bool,
    /// Negotiated ACP protocol version
    pub protocol_version: u16,
    pub agent_name: Option<String>,
    pub agent_version: Option<String>,
    pub cap_load_session: bool,
    pub cap_resume_session: bool,
    pub cap_prompt_image: bool,
    pub cap_prompt_audio: bool,
    pub cap_prompt_embedded_context: bool,
    pub cap_mcp_http: bool,
    pub cap_mcp_sse: bool,
}

// r[testability.agent-trait]
#[async_trait::async_trait]
pub trait AgentDriver: Send + Sync {
    async fn spawn(
        &self,
        kind: AgentKind,
        role: Role,
        config: &AgentSessionConfig,
    ) -> Result<AgentSpawnInfo, AgentError>;

    async fn prompt(
        &self,
        handle: &AgentHandle,
        parts: &[ship_types::PromptContentPart],
    ) -> Result<PromptResponse, AgentError>;

    async fn cancel(&self, handle: &AgentHandle) -> Result<(), AgentError>;

    fn notifications(
        &self,
        handle: &AgentHandle,
    ) -> Pin<Box<dyn Stream<Item = SessionEvent> + Send + '_>>;

    async fn resolve_permission(
        &self,
        handle: &AgentHandle,
        permission_id: &str,
        option_id: &str,
    ) -> Result<(), AgentError>;

    async fn set_model(&self, handle: &AgentHandle, model_id: &str) -> Result<(), AgentError>;

    async fn set_effort(
        &self,
        handle: &AgentHandle,
        config_id: &str,
        value_id: &str,
    ) -> Result<(), AgentError>;

    async fn kill(&self, handle: &AgentHandle) -> Result<(), AgentError>;
}
