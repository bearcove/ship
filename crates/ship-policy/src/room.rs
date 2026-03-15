use crate::{AgentRole, Participant, ParticipantKind};

/// A named communication space. Participants in a room can see each other's messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoomId(pub String);

/// The topology of rooms and participants in a Ship instance.
#[derive(Debug, Clone)]
pub struct Topology {
    pub human: Participant,
    pub admiral: Participant,
    pub sessions: Vec<SessionRoom>,
}

/// A single session room: one captain, one mate, working on a lane.
#[derive(Debug, Clone)]
pub struct SessionRoom {
    pub id: RoomId,
    pub captain: Participant,
    pub mate: Participant,
}

impl Topology {
    /// All participants visible in the admiral room:
    /// the admiral itself, plus all captains from all sessions.
    pub fn admiral_room_members(&self) -> Vec<&Participant> {
        let mut members: Vec<&Participant> = vec![&self.admiral];
        for session in &self.sessions {
            members.push(&session.captain);
        }
        members
    }

    /// All participants visible in a session room:
    /// the captain and the mate.
    pub fn session_room_members(&self, room: &RoomId) -> Option<Vec<&Participant>> {
        self.sessions
            .iter()
            .find(|s| &s.id == room)
            .map(|s| vec![&s.captain, &s.mate])
    }

    /// Find which session room a participant belongs to (by name).
    pub fn session_for_participant(&self, name: &str) -> Option<&SessionRoom> {
        self.sessions
            .iter()
            .find(|s| s.captain.name == name || s.mate.name == name)
    }

    /// Find a participant by name across the entire topology.
    pub fn find_participant(&self, name: &str) -> Option<&Participant> {
        if self.human.name == name {
            return Some(&self.human);
        }
        if self.admiral.name == name {
            return Some(&self.admiral);
        }
        for session in &self.sessions {
            if session.captain.name == name {
                return Some(&session.captain);
            }
            if session.mate.name == name {
                return Some(&session.mate);
            }
        }
        None
    }
}

/// Who can a given participant mention?
/// Returns the set of names this participant is allowed to address.
pub fn allowed_mentions(topology: &Topology, sender: &Participant) -> Vec<String> {
    match sender.kind {
        ParticipantKind::Human => {
            // Human can talk to the admiral directly, and to any captain
            let mut names = vec![topology.admiral.name.clone()];
            for session in &topology.sessions {
                names.push(session.captain.name.clone());
            }
            names
        }
        ParticipantKind::Agent(AgentRole::Admiral) => {
            // Admiral can mention any captain
            topology
                .sessions
                .iter()
                .map(|s| s.captain.name.clone())
                .collect()
        }
        ParticipantKind::Agent(AgentRole::Captain) => {
            // Captain can mention their mate and "@human" (which routes to admiral)
            let mut names = vec![];
            if let Some(session) = topology.session_for_participant(&sender.name) {
                names.push(session.mate.name.clone());
            }
            // Captain thinks they're talking to the human, but it routes to admiral
            names.push(topology.human.name.clone());
            names
        }
        ParticipantKind::Agent(AgentRole::Mate) => {
            // Mate can only mention their captain
            let mut names = vec![];
            if let Some(session) = topology.session_for_participant(&sender.name) {
                names.push(session.captain.name.clone());
            }
            names
        }
    }
}
