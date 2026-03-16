CREATE TABLE IF NOT EXISTS participants (
    -- Display name, e.g. 'Alex', 'Jordan', 'Amos'
    name TEXT PRIMARY KEY NOT NULL,
    -- 'human', 'admiral', 'captain', 'mate'
    kind TEXT NOT NULL CHECK (kind IN ('human', 'admiral', 'captain', 'mate'))
);

CREATE TABLE IF NOT EXISTS rooms (
    -- Room identifier, e.g. 'admiral' or 'session:<session_id>'
    id TEXT PRIMARY KEY NOT NULL,
    -- 'admiral' or 'session'
    kind TEXT NOT NULL CHECK (kind IN ('admiral', 'session')),
    -- For session rooms, links to the session. NULL for admiral room.
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_rooms_session ON rooms(session_id);

CREATE TABLE IF NOT EXISTS memberships (
    room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    participant_name TEXT NOT NULL REFERENCES participants(name) ON DELETE CASCADE,
    PRIMARY KEY (room_id, participant_name)
);

CREATE INDEX IF NOT EXISTS idx_memberships_participant ON memberships(participant_name);
