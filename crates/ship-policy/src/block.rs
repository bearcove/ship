use jiff::Timestamp;
use strid::braid;

use crate::RoomId;

/// Opaque block identifier, assigned by the system.
#[braid(rusqlite)]
pub struct BlockId;

/// A participant's display name. Links to the participants table.
#[braid(rusqlite, sailfish)]
pub struct ParticipantName;

/// A block in a room's feed. The fundamental unit of content that the
/// frontend renders. Blocks are created unsealed (still being built by
/// an agent) and sealed when finalized. Ship-policy only acts on sealed
/// blocks (for routing, mention extraction, etc.).
#[derive(Debug, Clone, facet::Facet)]
pub struct Block {
    pub id: BlockId,
    pub room_id: RoomId,
    /// Ordering within the room's feed.
    pub seq: u64,
    /// Who produced this block. None for system-generated blocks (errors, milestones).
    pub from: Option<ParticipantName>,
    /// Explicit recipient, if this is a directed message.
    pub to: Option<ParticipantName>,
    pub created_at: Timestamp,
    /// None while the block is still being built. Set when the block is finalized.
    pub sealed_at: Option<Timestamp>,
    pub content: BlockContent,
}

impl Block {
    pub fn is_sealed(&self) -> bool {
        self.sealed_at.is_some()
    }
}

/// The content of a block. Each variant corresponds to a distinct visual
/// representation in the frontend.
#[derive(Debug, Clone, facet::Facet)]
#[repr(u8)]
pub enum BlockContent {
    /// A text message in the feed.
    Text {
        text: String,
    },

    /// Agent's internal reasoning. The frontend shows a "thinking" indicator
    /// and reveals the text on hover.
    Thought {
        text: String,
    },

    /// A tool invocation by an agent.
    ToolCall {
        tool_name: String,
        status: ToolCallStatus,
        arguments: String,
        output: Option<String>,
        error: Option<String>,
        locations: Vec<ToolCallLocation>,
    },

    /// An update to the agent's work plan.
    PlanUpdate {
        steps: Vec<PlanStep>,
    },

    /// A permission request from an agent.
    Permission {
        tool_name: String,
        description: String,
        arguments: String,
        resolution: Option<PermissionResolution>,
    },

    /// An error encountered during processing.
    Error {
        message: String,
    },

    /// A significant workflow event (plan set, step committed, review submitted, etc.)
    Milestone {
        kind: MilestoneKind,
        title: String,
        summary: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
#[repr(u8)]
pub enum ToolCallStatus {
    Running,
    Success,
    Failure,
}

#[derive(Debug, Clone, facet::Facet)]
pub struct ToolCallLocation {
    pub path: String,
    pub line: Option<u64>,
}

#[derive(Debug, Clone, facet::Facet)]
pub struct PlanStep {
    pub description: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
#[repr(u8)]
pub enum PermissionResolution {
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
#[repr(u8)]
pub enum MilestoneKind {
    PlanSet,
    StepCommitted,
    ReviewSubmitted,
    TaskAccepted,
}
