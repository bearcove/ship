use strid::braid;

use crate::{AgentRole, Participant, ParticipantKind, ParticipantName, ParticipantNameRef};

/// A named communication space. Participants in a room can see each other's messages.
#[braid(rusqlite)]
pub struct RoomId;

/// A lane identifier. Lanes are long-lived work tracks (formerly "sessions").
#[braid(rusqlite)]
pub struct LaneId;

/// The topology of rooms and participants in a Ship instance.
#[derive(Debug, Clone)]
pub struct Topology {
    pub human: Participant,
    pub admiral: Participant,
    pub lanes: Vec<Lane>,
}

/// A lane: one captain, one mate, working together in a room.
/// Lanes are long-lived — tasks flow through them.
#[derive(Debug, Clone)]
pub struct Lane {
    pub id: RoomId,
    pub captain: Participant,
    pub mate: Participant,
}

impl Topology {
    /// All participants visible in the admiral room:
    /// the admiral itself, plus all captains from all lanes.
    pub fn admiral_room_members(&self) -> Vec<&Participant> {
        let mut members: Vec<&Participant> = vec![&self.admiral];
        for lane in &self.lanes {
            members.push(&lane.captain);
        }
        members
    }

    /// All participants visible in a lane's room:
    /// the captain and the mate.
    pub fn lane_members(&self, room: &RoomId) -> Option<Vec<&Participant>> {
        self.lanes
            .iter()
            .find(|l| &l.id == room)
            .map(|l| vec![&l.captain, &l.mate])
    }

    /// Find which lane a participant belongs to (by name).
    pub fn lane_for_participant(&self, name: &ParticipantNameRef) -> Option<&Lane> {
        self.lanes
            .iter()
            .find(|l| l.captain.name == *name || l.mate.name == *name)
    }

    /// Find a participant by name across the entire topology (exact match).
    pub fn find_participant(&self, name: &ParticipantNameRef) -> Option<&Participant> {
        self.all_participants().find(|p| p.name == *name)
    }

    /// Find a participant by name, case-insensitive.
    pub fn find_participant_ci(&self, name: &str) -> Option<&Participant> {
        self.all_participants()
            .find(|p| p.name.as_str().eq_ignore_ascii_case(name))
    }

    /// Check if any participant's name starts with the given prefix (case-insensitive).
    /// Used to detect incomplete mentions during streaming.
    pub fn any_name_starts_with(&self, prefix: &str) -> bool {
        let prefix_lower = prefix.to_ascii_lowercase();
        self.all_participants()
            .any(|p| p.name.as_str().to_ascii_lowercase().starts_with(&prefix_lower))
    }

    /// Iterate over all participants in the topology.
    fn all_participants(&self) -> impl Iterator<Item = &Participant> {
        std::iter::once(&self.human)
            .chain(std::iter::once(&self.admiral))
            .chain(
                self.lanes
                    .iter()
                    .flat_map(|l| [&l.captain, &l.mate]),
            )
    }
}

/// Who can a given participant mention?
/// Returns the set of names this participant is allowed to address.
pub fn allowed_mentions(topology: &Topology, sender: &Participant) -> Vec<ParticipantName> {
    match sender.kind {
        ParticipantKind::Human => {
            let mut names = vec![topology.admiral.name.clone()];
            for lane in &topology.lanes {
                names.push(lane.captain.name.clone());
            }
            names
        }
        ParticipantKind::Agent(AgentRole::Admiral) => {
            topology
                .lanes
                .iter()
                .map(|l| l.captain.name.clone())
                .collect()
        }
        ParticipantKind::Agent(AgentRole::Captain) => {
            let mut names = vec![];
            if let Some(lane) = topology.lane_for_participant(&sender.name) {
                names.push(lane.mate.name.clone());
            }
            names.push(topology.human.name.clone());
            names
        }
        ParticipantKind::Agent(AgentRole::Mate) => {
            let mut names = vec![];
            if let Some(lane) = topology.lane_for_participant(&sender.name) {
                names.push(lane.captain.name.clone());
            }
            names
        }
    }
}
