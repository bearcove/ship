use rusqlite::Connection;

/// Each entry is (name, sql). Order matters — they run sequentially.
/// Add new migrations at the end. Never reorder or remove existing ones.
const MIGRATIONS: &[(&str, &str)] = &[
    ("001_initial", include_str!("../migrations/001_initial.sql")),
    ("002_topology", include_str!("../migrations/002_topology.sql")),
    ("003_blocks", include_str!("../migrations/003_blocks.sql")),
];

const MIGRATIONS_TABLE_SQL: &str = "
CREATE TABLE IF NOT EXISTS __migrations (
    name TEXT PRIMARY KEY NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
";

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
    )?;

    conn.execute_batch(MIGRATIONS_TABLE_SQL)?;

    for (name, sql) in MIGRATIONS {
        let already_applied: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM __migrations WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?;

        if !already_applied {
            tracing::info!("applying migration: {name}");
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO __migrations (name) VALUES (?1)",
                [name],
            )?;
        }
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

        // Topology tables
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM participants", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM rooms", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memberships", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn init_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        init(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM __migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_are_recorded() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();

        let names: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM __migrations ORDER BY name")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };

        assert_eq!(names, vec!["001_initial", "002_topology", "003_blocks"]);
    }

    #[test]
    fn incremental_migration() {
        let conn = Connection::open_in_memory().unwrap();

        // Simulate a v1 database: only run the migrations table + first migration
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
        )
        .unwrap();
        conn.execute_batch(MIGRATIONS_TABLE_SQL).unwrap();
        conn.execute_batch(MIGRATIONS[0].1).unwrap();
        conn.execute(
            "INSERT INTO __migrations (name) VALUES (?1)",
            [MIGRATIONS[0].0],
        )
        .unwrap();

        // Now run init — should only apply migration 002
        init(&conn).unwrap();

        // Topology tables should exist
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM participants", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM __migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }
}
