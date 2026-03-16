mod agent;
mod events;
mod room;
mod runtime;

pub use agent::RuntimeRoomReader;
pub use events::{FrontendEvent, RoomSummary};
pub use room::{Feed, Room, RoomState};
pub use runtime::{ConnectSnapshot, Runtime, RuntimeError, RoomSnapshot};

// Re-export agent types that runtime consumers need.
pub use ship_agent::{AgentChannels, AgentConfig, AgentInput, AgentOutput, AgentStatus, ModelSpec};
