use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use rusqlite_facet::ConnectionFacetExt;

use crate::schema;

/// Error type for session store operations.
#[derive(Debug)]
pub struct StoreError {
    pub message: String,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for StoreError {}

/// Row types for rusqlite-facet queries.
#[derive(Debug, facet::Facet)]
struct SessionRow {
    data: String,
}

#[derive(Debug, facet::Facet)]
struct IdParam {
    id: String,
}

#[derive(Debug, facet::Facet)]
struct InsertParams {
    id: String,
    created_at: String,
    archived_at: Option<String>,
    title: Option<String>,
    is_read: i64,
    project: String,
    base_branch: String,
    branch_name: String,
    data: String,
}

/// Parsed session for listing — only reads the scalar columns needed for
/// filtering, plus the full JSON blob for deserialization.
#[derive(Debug, facet::Facet)]
struct ListRow {
    data: String,
    archived_at: Option<String>,
}

/// SQLite-backed session store.
///
/// Thread-safe via internal Mutex on the connection. All public methods
/// are synchronous — callers should use `spawn_blocking` from async code.
pub struct SqliteSessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteSessionStore {
    /// Open (or create) the session database at the given path.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError {
            message: format!("failed to open session database: {e}"),
        })?;
        schema::init(&conn).map_err(|e| StoreError {
            message: format!("failed to initialize schema: {e}"),
        })?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory session store (for testing).
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(|e| StoreError {
            message: format!("failed to open in-memory database: {e}"),
        })?;
        schema::init(&conn).map_err(|e| StoreError {
            message: format!("failed to initialize schema: {e}"),
        })?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Save a session. Upserts — creates or replaces.
    pub fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError> {
        let data = facet_json::to_string_pretty(session).map_err(|e| StoreError {
            message: format!("failed to serialize session {}: {e}", session.id.0),
        })?;

        let conn = self.conn.lock().expect("session db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT OR REPLACE INTO sessions (id, created_at, archived_at, title, is_read, project, base_branch, branch_name, data)
             VALUES (:id, :created_at, :archived_at, :title, :is_read, :project, :base_branch, :branch_name, :data)",
            &InsertParams {
                id: session.id.0.clone(),
                created_at: session.created_at.clone(),
                archived_at: session.archived_at.clone(),
                title: session.title.clone(),
                is_read: if session.is_read { 1 } else { 0 },
                project: session.config.project.0.clone(),
                base_branch: session.config.base_branch.clone(),
                branch_name: session.config.branch_name.clone(),
                data,
            },
        )
        .map_err(|e| StoreError {
            message: format!("failed to save session {}: {e}", session.id.0),
        })?;

        Ok(())
    }

    /// Load a single session by ID.
    pub fn load_session(&self, id: &str) -> Result<Option<PersistedSession>, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");
        let row: Option<SessionRow> = conn
            .facet_query_optional_ref(
                "SELECT data FROM sessions WHERE id = :id",
                &IdParam {
                    id: id.to_owned(),
                },
            )
            .map_err(|e| StoreError {
                message: format!("failed to load session {id}: {e}"),
            })?;

        match row {
            None => Ok(None),
            Some(row) => {
                let session = deserialize_session(&row.data, id)?;
                Ok(Some(session))
            }
        }
    }

    /// List all non-archived sessions.
    pub fn list_sessions(&self) -> Result<Vec<PersistedSession>, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");
        let rows: Vec<ListRow> = conn
            .facet_query(
                "SELECT data, archived_at FROM sessions WHERE archived_at IS NULL",
                (),
            )
            .map_err(|e| StoreError {
                message: format!("failed to list sessions: {e}"),
            })?;

        let mut sessions = Vec::with_capacity(rows.len());
        for row in rows {
            match deserialize_session(&row.data, "<list>") {
                Ok(session) => sessions.push(session),
                Err(e) => {
                    tracing::warn!("skipping unparseable session: {}", e.message);
                }
            }
        }
        Ok(sessions)
    }

    /// Delete a session by ID.
    pub fn delete_session(&self, id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");
        conn.facet_execute_ref(
            "DELETE FROM sessions WHERE id = :id",
            &IdParam {
                id: id.to_owned(),
            },
        )
        .map_err(|e| StoreError {
            message: format!("failed to delete session {id}: {e}"),
        })?;
        Ok(())
    }
}

use ship_types::PersistedSession;

fn deserialize_session(json: &str, display_name: &str) -> Result<PersistedSession, StoreError> {
    facet_json::from_str::<PersistedSession>(json).map_err(|e| StoreError {
        message: format!("failed to deserialize session {display_name}: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ship_types::*;

    fn make_test_session(id: &str, project: &str) -> PersistedSession {
        PersistedSession {
            id: SessionId(id.to_owned()),
            created_at: "2025-01-01T00:00:00Z".to_owned(),
            config: SessionConfig {
                project: ProjectName(project.to_owned()),
                base_branch: "main".to_owned(),
                branch_name: format!("ship/{id}/task"),
                captain_kind: AgentKind::Claude,
                mate_kind: AgentKind::Claude,
                captain_preset_id: None,
                mate_preset_id: None,
                captain_provider: None,
                mate_provider: None,
                captain_model_id: None,
                mate_model_id: None,
                autonomy_mode: AutonomyMode::HumanInTheLoop,
                mcp_servers: Vec::new(),
            },
            captain: AgentSnapshot {
                role: Role::Captain,
                kind: AgentKind::Claude,
                state: AgentState::Idle,
                context_remaining_percent: None,
                preset_id: None,
                provider: None,
                model_id: None,
                available_models: Vec::new(),
                effort_config_id: None,
                effort_value_id: None,
                available_effort_values: Vec::new(),
            },
            mate: AgentSnapshot {
                role: Role::Mate,
                kind: AgentKind::Claude,
                state: AgentState::Idle,
                context_remaining_percent: None,
                preset_id: None,
                provider: None,
                model_id: None,
                available_models: Vec::new(),
                effort_config_id: None,
                effort_value_id: None,
                available_effort_values: Vec::new(),
            },
            startup_state: SessionStartupState::Ready,
            session_event_log: Vec::new(),
            current_task: None,
            task_history: Vec::new(),
            title: Some("Test session".to_owned()),
            archived_at: None,
            captain_acp_session_id: None,
            mate_acp_session_id: None,
            is_read: false,
            captain_has_ever_assigned: false,
            captain_delegation_reminded: false,
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        let session = make_test_session("sess-001", "myproject");

        store.save_session(&session).unwrap();

        let loaded = store.load_session("sess-001").unwrap().unwrap();
        assert_eq!(loaded.id.0, "sess-001");
        assert_eq!(loaded.config.project.0, "myproject");
        assert_eq!(loaded.title.as_deref(), Some("Test session"));
        assert!(!loaded.is_read);
    }

    #[test]
    fn load_missing_returns_none() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        let loaded = store.load_session("nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn list_excludes_archived() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        let active = make_test_session("sess-active", "proj");
        store.save_session(&active).unwrap();

        let mut archived = make_test_session("sess-archived", "proj");
        archived.archived_at = Some("2025-06-01T00:00:00Z".to_owned());
        store.save_session(&archived).unwrap();

        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id.0, "sess-active");
    }

    #[test]
    fn save_upserts() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        let mut session = make_test_session("sess-001", "proj");
        store.save_session(&session).unwrap();

        session.title = Some("Updated title".to_owned());
        session.is_read = true;
        store.save_session(&session).unwrap();

        let loaded = store.load_session("sess-001").unwrap().unwrap();
        assert_eq!(loaded.title.as_deref(), Some("Updated title"));
        assert!(loaded.is_read);

        // Only one row
        let all = store.list_sessions().unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn delete_session() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        let session = make_test_session("sess-001", "proj");
        store.save_session(&session).unwrap();

        store.delete_session("sess-001").unwrap();

        let loaded = store.load_session("sess-001").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        store.delete_session("nonexistent").unwrap();
    }

    #[test]
    fn list_multiple_projects() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        store
            .save_session(&make_test_session("s1", "project-a"))
            .unwrap();
        store
            .save_session(&make_test_session("s2", "project-b"))
            .unwrap();
        store
            .save_session(&make_test_session("s3", "project-a"))
            .unwrap();

        let all = store.list_sessions().unwrap();
        assert_eq!(all.len(), 3);
    }
}
