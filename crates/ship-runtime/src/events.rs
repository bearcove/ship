use ship_policy::{Block, BlockId, RoomId, Task, TaskPhase};

/// Events sent to the frontend over a single stream.
/// Each event carries a full record — no patches.
#[derive(Debug, Clone)]
pub enum FrontendEvent {
    /// A block was created, updated, or sealed. Full block included.
    BlockChanged { room_id: RoomId, block: Block },

    /// A block was removed.
    BlockRemoved { room_id: RoomId, block_id: BlockId },

    /// A task was created or its state changed. Full task included.
    TaskChanged { room_id: RoomId, task: Task },

    /// The lane's active task was cleared (task completed or cancelled).
    TaskCleared { room_id: RoomId },

    /// Room-level summary changed (sidebar data).
    RoomChanged { summary: RoomSummary },

    /// A room was removed from the topology.
    RoomRemoved { room_id: RoomId },
}

/// Sidebar-level summary of a room. The frontend uses this to render
/// the room list without needing to inspect individual blocks.
#[derive(Debug, Clone)]
pub struct RoomSummary {
    pub room_id: RoomId,
    /// Display name for the room (e.g. "Alex & Jordan").
    pub display_name: String,
    /// Current task title, if any.
    pub task_title: Option<String>,
    /// Current task phase, if any.
    pub task_phase: Option<TaskPhase>,
    /// Plan progress: (completed, total). None if no plan.
    pub plan_progress: Option<(usize, usize)>,
}
