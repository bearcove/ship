mod model_spec;
mod run;

use std::future::Future;

use camino::Utf8PathBuf;
use ship_policy::{AgentRole, Block, BlockContent, BlockId, Delivery, ParticipantName, RoomId};
use tokio::sync::mpsc;

/// Read-only access to a room's history, for context rebuilding.
pub trait RoomReader: Send + Sync {
    fn recent_blocks(
        &self,
        room_id: &RoomId,
        limit: usize,
    ) -> impl Future<Output = Vec<Block>> + Send;
}

/// What the runtime sends to an agent.
pub enum AgentInput {
    /// A delivery routed to this agent by policy.
    Delivery(Delivery),
    /// Change the agent/model, e.g. `claude::opus` or `codex::gpt-5.4-high`.
    /// If the agent kind changes, the agent process is restarted transparently.
    SetModel(ModelSpec),
    /// Shut down the agent cleanly.
    Shutdown,
}

/// What the agent sends back to the runtime.
pub enum AgentOutput {
    /// Agent produced or updated block content.
    UpdateBlock {
        block_id: BlockId,
        content: BlockContent,
    },
    /// Agent status changed.
    StatusChanged(AgentStatus),
}

/// Current state of an agent.
pub enum AgentStatus {
    /// Agent is waiting for input.
    Idle,
    /// Agent is processing a prompt.
    Prompting,
    /// Agent process died.
    Dead { error: String },
    /// Context window usage report.
    ContextUsage { used_pct: u8 },
}

/// Channel endpoints for communicating with a running agent.
pub struct AgentChannels {
    pub tx: mpsc::Sender<AgentInput>,
    pub rx: mpsc::Receiver<AgentOutput>,
}

/// Configuration for spawning an agent.
pub struct AgentConfig {
    pub room_id: RoomId,
    pub participant: ParticipantName,
    pub role: AgentRole,
    pub model_spec: ModelSpec,
    pub system_prompt: String,
    pub mcp_servers: Vec<ship_types::McpServerConfig>,
    pub worktree_path: Utf8PathBuf,
}

pub use model_spec::ModelSpec;
pub use run::spawn_agent;
