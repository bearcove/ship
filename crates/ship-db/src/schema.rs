use rusqlite::Connection;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA_SQL: &str = "
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
";

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;")?;

    let current_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if current_version < SCHEMA_VERSION {
        if current_version > 0 {
            // Future: migrate from older versions. For now, drop and recreate.
            conn.execute_batch(
                "DROP TABLE IF EXISTS activity_log;
                 DROP TABLE IF EXISTS session_events;
                 DROP TABLE IF EXISTS sessions;",
            )?;
        }
        conn.execute_batch(SCHEMA_SQL)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();

        // Verify tables exist by querying them
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM session_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM activity_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn init_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        init(&conn).unwrap();

        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }
}
