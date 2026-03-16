use roam::Tx;
use ship_policy::{
    Block, ParticipantName, RoomId, Task, Topology,
};

/// Snapshot of a single room's state, sent on connect.
#[derive(Debug, Clone, facet::Facet)]
pub struct RoomSnapshot {
    pub room_id: RoomId,
    pub current_task: Option<Task>,
    pub recent_blocks: Vec<Block>,
}

/// Everything the frontend needs on connect.
#[derive(Debug, Clone, facet::Facet)]
pub struct ConnectSnapshot {
    pub topology: Topology,
    pub rooms: Vec<RoomSnapshot>,
}

/// An event pushed to the frontend.
#[derive(Debug, Clone, facet::Facet)]
#[repr(u8)]
pub enum FrontendEvent {
    BlockChanged {
        room_id: RoomId,
        block: Block,
    },
    TaskChanged {
        room_id: RoomId,
        task: Task,
    },
    TaskCleared {
        room_id: RoomId,
    },
    TopologyChanged {
        topology: Topology,
    },
}

/// Human input submitted from the frontend.
#[derive(Debug, Clone, facet::Facet)]
pub struct HumanInput {
    pub room_id: RoomId,
    pub text: String,
}

// r[frontend.rpc]
#[roam::service]
pub trait Frontend {
    /// Get the full state snapshot (topology + rooms + tasks + recent blocks).
    async fn connect(&self) -> ConnectSnapshot;

    /// Subscribe to real-time events.
    async fn subscribe(&self, events: Tx<FrontendEvent>);

    /// Submit human input to a room (human writes a message).
    async fn send_message(&self, input: HumanInput);

    /// Change an agent's model (e.g. `claude::opus`).
    async fn set_agent_model(&self, participant: ParticipantName, model_spec: String);
}
