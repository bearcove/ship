use ship_db::ShipDb;
use ship_policy::Topology;

use crate::room::Room;

/// The ship runtime: manages the topology of rooms, routes messages
/// through policy, persists to ship-db, and broadcasts to subscribers.
pub struct Runtime {
    db: ShipDb,
    topology: Topology,
    rooms: Vec<Room>,
}

impl Runtime {
    /// Create a new runtime backed by the given database.
    /// Loads topology from db if one exists.
    pub fn new(db: ShipDb) -> Self {
        let topology = db
            .load_topology()
            .ok()
            .flatten()
            .unwrap_or_else(empty_topology);
        let rooms = topology
            .sessions
            .iter()
            .map(|s| Room::cold(s.id.clone()))
            .collect();
        Self {
            db,
            topology,
            rooms,
        }
    }
}

fn empty_topology() -> Topology {
    Topology {
        human: ship_policy::Participant::human("Human"),
        admiral: ship_policy::Participant::agent("Admiral", ship_policy::AgentRole::Admiral),
        sessions: vec![],
    }
}
