/// The role an agent plays. Determines tool access and behavioral constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentRole {
    /// Coordinates work, reviews, merges. Present in both session and admiral rooms.
    Captain,
    /// Implements tasks assigned by the captain. Only present in session room.
    Mate,
    /// Coordinates across sessions, buffers human from interrupts. Only in admiral room.
    Admiral,
}

/// A participant in the system. Could be an agent or the human.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Participant {
    /// Display name, e.g. "Alex", "Jordan", "Morgan", "Amos"
    pub name: String,
    /// What kind of participant this is
    pub kind: ParticipantKind,
}

/// Whether a participant is an agent (with a role) or the human.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticipantKind {
    Agent(AgentRole),
    Human,
}

impl Participant {
    pub fn agent(name: impl Into<String>, role: AgentRole) -> Self {
        Self {
            name: name.into(),
            kind: ParticipantKind::Agent(role),
        }
    }

    pub fn human(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: ParticipantKind::Human,
        }
    }

    pub fn role(&self) -> Option<AgentRole> {
        match self.kind {
            ParticipantKind::Agent(role) => Some(role),
            ParticipantKind::Human => None,
        }
    }

    pub fn is_human(&self) -> bool {
        matches!(self.kind, ParticipantKind::Human)
    }
}
