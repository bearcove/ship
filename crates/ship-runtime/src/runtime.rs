use ship_db::ShipDb;
use ship_policy::{
    AgentRole, Block, BlockContent, BlockId, Delivery, Participant, ParticipantName, RoomId,
    Topology,
};

use crate::room::{Feed, Room, RoomState};

/// In-memory room state. Borrowed independently from the db.
struct Rooms {
    rooms: Vec<Room>,
}

impl Rooms {
    fn new(topology: &Topology) -> Self {
        let rooms = topology
            .sessions
            .iter()
            .map(|s| Room::cold(s.id.clone()))
            .collect();
        Self { rooms }
    }

    fn get_mut(&mut self, room_id: &RoomId) -> Option<&mut Room> {
        self.rooms.iter_mut().find(|r| &r.id == room_id)
    }

    /// Ensure a room is warm, hydrating from db if needed.
    fn ensure_warm(&mut self, room_id: &RoomId, db: &ShipDb) -> Result<&mut Feed, RuntimeError> {
        let room = self
            .get_mut(room_id)
            .ok_or_else(|| RuntimeError::RoomNotFound(room_id.clone()))?;

        if matches!(room.state, RoomState::Cold) {
            let blocks = db.list_blocks(room_id).map_err(RuntimeError::Db)?;
            let feed = room.ensure_warm();
            feed.hydrate(blocks);
        }

        Ok(room.feed_mut().expect("just warmed"))
    }
}

/// The ship runtime: manages the topology of rooms, routes messages
/// through policy, persists to ship-db, and broadcasts to subscribers.
pub struct Runtime {
    db: ShipDb,
    topology: Topology,
    rooms: Rooms,
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
        let rooms = Rooms::new(&topology);
        Self {
            db,
            topology,
            rooms,
        }
    }

    /// Initialize a fresh topology and persist it.
    pub fn init_topology(&mut self, topology: Topology) -> Result<(), ship_db::StoreError> {
        self.db.save_topology(&topology)?;
        self.rooms = Rooms::new(&topology);
        self.topology = topology;
        Ok(())
    }

    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    /// Open a new unsealed block in a room. Persists to db immediately.
    ///
    /// Validates that the sender (if any) is a member of the room.
    pub fn open_block(
        &mut self,
        room_id: &RoomId,
        from: Option<ParticipantName>,
        to: Option<ParticipantName>,
        content: BlockContent,
    ) -> Result<BlockId, RuntimeError> {
        if let Some(ref sender_name) = from {
            self.check_room_membership(room_id, sender_name)?;
        }
        let feed = self.rooms.ensure_warm(room_id, &self.db)?;
        let block = feed.open_block(room_id.clone(), from, to, content);
        let block_clone = block.clone();
        self.db.insert_block(&block_clone).map_err(RuntimeError::Db)?;
        Ok(block_clone.id)
    }

    /// Seal a block (mark it as finalized). Persists to db.
    ///
    /// After sealing, runs the block through policy: parses mentions,
    /// checks allowed_mentions, and routes deliveries.
    /// Returns the list of deliveries produced (may be empty).
    pub fn seal_block(
        &mut self,
        room_id: &RoomId,
        block_id: &BlockId,
    ) -> Result<Vec<Delivery>, RuntimeError> {
        let feed = self.rooms.ensure_warm(room_id, &self.db)?;
        let block = feed
            .seal_block(block_id)
            .ok_or_else(|| RuntimeError::BlockNotFound(block_id.clone()))?;
        let sealed_at = block.sealed_at.expect("just sealed");
        let content = block.content.clone();
        let from_name = block.from.as_ref().map(|p| p.to_string());
        self.db
            .seal_block(block_id, sealed_at, &content)
            .map_err(RuntimeError::Db)?;

        let deliveries = self.route_sealed_block(&content, from_name.as_deref());
        Ok(deliveries)
    }

    /// Check that a participant is a member of the given room.
    fn check_room_membership(
        &self,
        room_id: &RoomId,
        sender: &ParticipantName,
    ) -> Result<(), RuntimeError> {
        let members = self.topology.session_room_members(room_id);
        let is_member = members
            .as_ref()
            .is_some_and(|m| m.iter().any(|p| p.name == sender.as_str()));
        if !is_member {
            return Err(RuntimeError::NotAMember {
                participant: sender.clone(),
                room: room_id.clone(),
            });
        }
        Ok(())
    }

    /// Parse mentions and route deliveries for a sealed block.
    fn route_sealed_block(
        &self,
        content: &BlockContent,
        from_name: Option<&str>,
    ) -> Vec<Delivery> {
        let Some(from_name) = from_name else {
            return vec![];
        };

        let text = match content {
            BlockContent::Text { text } => text,
            _ => return vec![],
        };

        let mention = ship_policy::parse_mention(text, &self.topology);
        match mention {
            ship_policy::ParsedMention::Found { name, rest } => {
                let sender = self.topology.find_participant(from_name);
                let allowed = sender
                    .map(|s| ship_policy::allowed_mentions(&self.topology, s))
                    .unwrap_or_default();

                if allowed.iter().any(|a| a == &name) {
                    let action = ship_policy::Action::MessageSent {
                        from: from_name.to_owned(),
                        mention: name,
                        text: rest,
                    };
                    ship_policy::route(&action, &self.topology)
                } else {
                    vec![]
                }
            }
            ship_policy::ParsedMention::None => {
                let action = ship_policy::Action::UnaddressedMessage {
                    from: from_name.to_owned(),
                    text: text.clone(),
                };
                ship_policy::route(&action, &self.topology)
            }
            _ => vec![],
        }
    }

    /// Update an unsealed block's content. Persists to db.
    pub fn update_block(
        &mut self,
        room_id: &RoomId,
        block_id: &BlockId,
        content: BlockContent,
    ) -> Result<(), RuntimeError> {
        self.db
            .update_block_content(block_id, &content)
            .map_err(RuntimeError::Db)?;
        let feed = self.rooms.ensure_warm(room_id, &self.db)?;
        feed.update_block(block_id, content);
        Ok(())
    }

    /// Get the feed blocks for a room (warming it if needed).
    pub fn blocks(&mut self, room_id: &RoomId) -> Result<&[Block], RuntimeError> {
        let feed = self.rooms.ensure_warm(room_id, &self.db)?;
        Ok(feed.blocks())
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    Db(ship_db::StoreError),
    RoomNotFound(RoomId),
    BlockNotFound(BlockId),
    NotAMember {
        participant: ParticipantName,
        room: RoomId,
    },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Db(e) => write!(f, "database error: {e}"),
            Self::RoomNotFound(id) => write!(f, "room not found: {id}"),
            Self::BlockNotFound(id) => write!(f, "block not found: {id}"),
            Self::NotAMember { participant, room } => {
                write!(f, "{participant} is not a member of room {room}")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

fn empty_topology() -> Topology {
    Topology {
        human: Participant::human("Human"),
        admiral: Participant::agent("Admiral", AgentRole::Admiral),
        sessions: vec![],
    }
}
