use strid::braid;

use crate::{AgentRole, Participant, ParticipantKind};

/// A named communication space. Participants in a room can see each other's messages.
#[braid(rusqlite)]
pub struct RoomId;

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

    /// Find a participant by name across the entire topology (exact match).
    pub fn find_participant(&self, name: &str) -> Option<&Participant> {
        self.all_participants().find(|p| p.name == name)
    }

    /// Find a participant by name, case-insensitive.
    pub fn find_participant_ci(&self, name: &str) -> Option<&Participant> {
        self.all_participants()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Check if any participant's name starts with the given prefix (case-insensitive).
    /// Used to detect incomplete mentions during streaming.
    pub fn any_name_starts_with(&self, prefix: &str) -> bool {
        let prefix_lower = prefix.to_ascii_lowercase();
        self.all_participants()
            .any(|p| p.name.to_ascii_lowercase().starts_with(&prefix_lower))
    }

    /// Iterate over all participants in the topology.
    fn all_participants(&self) -> impl Iterator<Item = &Participant> {
        std::iter::once(&self.human)
            .chain(std::iter::once(&self.admiral))
            .chain(
                self.sessions
                    .iter()
                    .flat_map(|s| [&s.captain, &s.mate]),
            )
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
