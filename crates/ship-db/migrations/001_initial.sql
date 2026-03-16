CREATE TABLE IF NOT EXISTS sessions (
    -- Primary key: ULID string
    id TEXT PRIMARY KEY NOT NULL,

    -- Scalar fields for fast queries/filtering
    created_at TEXT NOT NULL DEFAULT '',
    archived_at TEXT,
    title TEXT,
    is_read INTEGER NOT NULL DEFAULT 0,

    -- Session config (queryable scalars pulled out)
    project TEXT NOT NULL,
    base_branch TEXT NOT NULL,
    branch_name TEXT NOT NULL,
    workflow TEXT NOT NULL DEFAULT 'merge',

    -- Full session data as JSON blob (the complete PersistedSession)
    -- This is the source of truth — scalar columns above are denormalized
    -- for query performance.
    data TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project);
CREATE INDEX IF NOT EXISTS idx_sessions_archived ON sessions(archived_at);
CREATE INDEX IF NOT EXISTS idx_sessions_created ON sessions(created_at);

CREATE TABLE IF NOT EXISTS session_events (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    timestamp TEXT NOT NULL,
    -- The SessionEventEnvelope as JSON
    data TEXT NOT NULL,
    PRIMARY KEY (session_id, seq)
);

CREATE TABLE IF NOT EXISTS activity_log (
    id INTEGER PRIMARY KEY NOT NULL,
    timestamp TEXT NOT NULL,
    session_id TEXT NOT NULL,
    session_slug TEXT NOT NULL,
    session_title TEXT,
    -- The ActivityKind as JSON
    kind TEXT NOT NULL
);
