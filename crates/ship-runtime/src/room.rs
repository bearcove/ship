use jiff::Timestamp;
use ship_policy::{Block, BlockContent, BlockId, ParticipantName, RoomId};
use ulid::Ulid;

/// A room's feed: the ordered sequence of blocks.
pub struct Feed {
    blocks: Vec<Block>,
    next_seq: u64,
}

impl Feed {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            next_seq: 0,
        }
    }

    /// Hydrate from persisted blocks (called when loading a room from db).
    pub fn hydrate(&mut self, blocks: Vec<Block>) {
        self.next_seq = blocks.last().map_or(0, |b| b.seq + 1);
        self.blocks = blocks;
    }

    /// Create a new unsealed block in this feed.
    pub fn open_block(
        &mut self,
        room_id: RoomId,
        from: Option<ParticipantName>,
        to: Option<ParticipantName>,
        content: BlockContent,
    ) -> &Block {
        let seq = self.next_seq;
        self.next_seq += 1;
        let block = Block {
            id: BlockId::new(Ulid::new().to_string()),
            room_id,
            seq,
            from,
            to,
            created_at: Timestamp::now(),
            sealed_at: None,
            content,
        };
        self.blocks.push(block);
        self.blocks.last().unwrap()
    }

    /// Seal the block with the given id (mark it as finalized).
    pub fn seal_block(&mut self, id: &BlockId) -> Option<&Block> {
        let block = self.blocks.iter_mut().find(|b| &b.id == id)?;
        block.sealed_at = Some(Timestamp::now());
        self.blocks.iter().rfind(|b| &b.id == id)
    }

    /// Update the content of an unsealed block (e.g. text append during streaming).
    pub fn update_block(&mut self, id: &BlockId, content: BlockContent) -> Option<&Block> {
        let block = self.blocks.iter_mut().find(|b| &b.id == id && !b.is_sealed())?;
        block.content = content;
        Some(block)
    }

    /// All blocks so far (for replay on connect).
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// The state of a room: cold (metadata only) or warm (feed loaded).
pub enum RoomState {
    /// We know the room exists but haven't loaded its feed.
    Cold,
    /// Feed is loaded and active.
    Warm(Feed),
}

/// A room with its current hydration state.
pub struct Room {
    pub id: RoomId,
    pub state: RoomState,
}

impl Room {
    pub fn cold(id: RoomId) -> Self {
        Self {
            id,
            state: RoomState::Cold,
        }
    }

    pub fn warm(id: RoomId) -> Self {
        Self {
            id,
            state: RoomState::Warm(Feed::new()),
        }
    }

    /// Ensure the room is warm. Returns a mutable ref to the feed.
    pub fn ensure_warm(&mut self) -> &mut Feed {
        if matches!(self.state, RoomState::Cold) {
            self.state = RoomState::Warm(Feed::new());
        }
        match &mut self.state {
            RoomState::Warm(feed) => feed,
            RoomState::Cold => unreachable!(),
        }
    }

    pub fn feed(&self) -> Option<&Feed> {
        match &self.state {
            RoomState::Warm(feed) => Some(feed),
            RoomState::Cold => None,
        }
    }

    pub fn feed_mut(&mut self) -> Option<&mut Feed> {
        match &mut self.state {
            RoomState::Warm(feed) => Some(feed),
            RoomState::Cold => None,
        }
    }
}
