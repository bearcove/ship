use jiff::Timestamp;
use strid::braid;

use crate::{RoomId, TaskPhase};

/// Unique identifier for a task.
#[braid(rusqlite)]
pub struct TaskId;

/// A task: a unit of work that flows through a lane.
/// Tasks have a lifecycle governed by TaskPhase transitions.
#[derive(Debug, Clone, facet::Facet)]
pub struct Task {
    pub id: TaskId,
    /// The room (lane) this task belongs to.
    pub room_id: RoomId,
    /// Short title shown in the UI sidebar.
    pub title: String,
    /// Full description with all details.
    pub description: String,
    /// Current lifecycle phase.
    pub phase: TaskPhase,
    pub created_at: Timestamp,
    /// Set when the task reaches a terminal phase (Accepted/Cancelled).
    pub completed_at: Option<Timestamp>,
    /// Cumulative lines added across all commits in this task.
    pub lines_added: u64,
    /// Cumulative lines removed across all commits in this task.
    pub lines_removed: u64,
    /// Number of commits made for this task.
    pub commit_count: u32,
}
