CREATE TABLE IF NOT EXISTS blocks (
    -- Opaque block ID (e.g. ULID)
    id TEXT PRIMARY KEY NOT NULL,
    -- Which room this block belongs to
    room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    -- Ordering within the room's feed
    seq INTEGER NOT NULL,
    -- Who produced this block (participant name). NULL for system blocks.
    from_participant TEXT REFERENCES participants(name),
    -- Explicit recipient, if directed
    to_participant TEXT REFERENCES participants(name),
    -- When the block was created
    created_at TEXT NOT NULL,
    -- NULL while being built, set when finalized
    sealed_at TEXT,
    -- BlockContent as JSON
    content TEXT NOT NULL,

    UNIQUE (room_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_blocks_room ON blocks(room_id);
CREATE INDEX IF NOT EXISTS idx_blocks_room_seq ON blocks(room_id, seq);
CREATE INDEX IF NOT EXISTS idx_blocks_from ON blocks(from_participant);
CREATE INDEX IF NOT EXISTS idx_blocks_sealed ON blocks(sealed_at);
