use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use rusqlite_facet::ConnectionFacetExt;

use crate::schema;
use ship_types::{ActivityEntry, PersistedSession, SessionEventEnvelope};

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

// ── Row/param types for rusqlite-facet ──────────────────────────────────

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
    workflow: String,
    data: String,
}

#[derive(Debug, facet::Facet)]
struct ListRow {
    data: String,
    archived_at: Option<String>,
}

#[derive(Debug, facet::Facet)]
struct ProjectParam {
    project: String,
}

#[derive(Debug, facet::Facet)]
struct ArchiveParams {
    id: String,
    archived_at: Option<String>,
    data: String,
}

#[derive(Debug, facet::Facet)]
struct EventInsertParams {
    session_id: String,
    seq: i64,
    timestamp: String,
    data: String,
}

#[derive(Debug, facet::Facet)]
struct SessionIdParam {
    session_id: String,
}

#[derive(Debug, facet::Facet)]
struct EventRow {
    data: String,
}

#[derive(Debug, facet::Facet)]
struct ActivityInsertParams {
    timestamp: String,
    session_id: String,
    session_slug: String,
    session_title: Option<String>,
    kind: String,
}

#[derive(Debug, facet::Facet)]
struct ActivityRow {
    id: i64,
    timestamp: String,
    session_id: String,
    session_slug: String,
    session_title: Option<String>,
    kind: String,
}

#[derive(Debug, facet::Facet)]
struct CountRow {
    count: i64,
}

#[derive(Debug, facet::Facet)]
struct LimitParam {
    limit: i64,
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

    // ── Session CRUD ────────────────────────────────────────────────────

    /// Save a session. Upserts — creates or replaces.
    pub fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError> {
        let data = facet_json::to_string_pretty(session).map_err(|e| StoreError {
            message: format!("failed to serialize session {}: {e}", session.id.0),
        })?;

        let conn = self.conn.lock().expect("session db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT OR REPLACE INTO sessions (id, created_at, archived_at, title, is_read, project, base_branch, branch_name, workflow, data)
             VALUES (:id, :created_at, :archived_at, :title, :is_read, :project, :base_branch, :branch_name, :workflow, :data)",
            &InsertParams {
                id: session.id.0.clone(),
                created_at: session.created_at.clone(),
                archived_at: session.archived_at.clone(),
                title: session.title.clone(),
                is_read: if session.is_read { 1 } else { 0 },
                project: session.config.project.0.clone(),
                base_branch: session.config.base_branch.clone(),
                branch_name: session.config.branch_name.clone(),
                workflow: workflow_to_str(session.config.workflow),
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

        collect_sessions(rows.iter().map(|r| r.data.as_str()))
    }

    /// List all non-archived sessions for a specific project.
    pub fn list_sessions_for_project(
        &self,
        project: &str,
    ) -> Result<Vec<PersistedSession>, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");
        let rows: Vec<ListRow> = conn
            .facet_query_ref(
                "SELECT data, archived_at FROM sessions WHERE archived_at IS NULL AND project = :project",
                &ProjectParam {
                    project: project.to_owned(),
                },
            )
            .map_err(|e| StoreError {
                message: format!("failed to list sessions for project {project}: {e}"),
            })?;

        collect_sessions(rows.iter().map(|r| r.data.as_str()))
    }

    /// Delete a session by ID. Also deletes associated events (ON DELETE CASCADE).
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

    // ── Archive / unarchive ─────────────────────────────────────────────

    /// Archive a session. Sets `archived_at` on both the scalar column and
    /// inside the JSON blob, then persists.
    pub fn archive_session(&self, id: &str, timestamp: &str) -> Result<bool, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");

        let row: Option<SessionRow> = conn
            .facet_query_optional_ref(
                "SELECT data FROM sessions WHERE id = :id",
                &IdParam { id: id.to_owned() },
            )
            .map_err(|e| StoreError {
                message: format!("failed to load session {id} for archive: {e}"),
            })?;

        let Some(row) = row else {
            return Ok(false);
        };

        let mut session = deserialize_session(&row.data, id)?;
        session.archived_at = Some(timestamp.to_owned());

        let data = facet_json::to_string_pretty(&session).map_err(|e| StoreError {
            message: format!("failed to serialize session {id}: {e}"),
        })?;

        conn.facet_execute_ref(
            "UPDATE sessions SET archived_at = :archived_at, data = :data WHERE id = :id",
            &ArchiveParams {
                id: id.to_owned(),
                archived_at: Some(timestamp.to_owned()),
                data,
            },
        )
        .map_err(|e| StoreError {
            message: format!("failed to archive session {id}: {e}"),
        })?;

        Ok(true)
    }

    /// Unarchive a session. Clears `archived_at` on both the scalar column
    /// and inside the JSON blob.
    pub fn unarchive_session(&self, id: &str) -> Result<bool, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");

        let row: Option<SessionRow> = conn
            .facet_query_optional_ref(
                "SELECT data FROM sessions WHERE id = :id",
                &IdParam { id: id.to_owned() },
            )
            .map_err(|e| StoreError {
                message: format!("failed to load session {id} for unarchive: {e}"),
            })?;

        let Some(row) = row else {
            return Ok(false);
        };

        let mut session = deserialize_session(&row.data, id)?;
        session.archived_at = None;

        let data = facet_json::to_string_pretty(&session).map_err(|e| StoreError {
            message: format!("failed to serialize session {id}: {e}"),
        })?;

        conn.facet_execute_ref(
            "UPDATE sessions SET archived_at = :archived_at, data = :data WHERE id = :id",
            &ArchiveParams {
                id: id.to_owned(),
                archived_at: None,
                data,
            },
        )
        .map_err(|e| StoreError {
            message: format!("failed to unarchive session {id}: {e}"),
        })?;

        Ok(true)
    }

    // ── Session events ──────────────────────────────────────────────────

    /// Append an event to the session_events table.
    pub fn append_event(
        &self,
        session_id: &str,
        envelope: &SessionEventEnvelope,
    ) -> Result<(), StoreError> {
        let data = facet_json::to_string_pretty(envelope).map_err(|e| StoreError {
            message: format!("failed to serialize event seq={} for {session_id}: {e}", envelope.seq),
        })?;

        let conn = self.conn.lock().expect("session db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT INTO session_events (session_id, seq, timestamp, data)
             VALUES (:session_id, :seq, :timestamp, :data)",
            &EventInsertParams {
                session_id: session_id.to_owned(),
                seq: envelope.seq as i64,
                timestamp: envelope.timestamp.clone(),
                data,
            },
        )
        .map_err(|e| StoreError {
            message: format!(
                "failed to append event seq={} for session {session_id}: {e}",
                envelope.seq
            ),
        })?;

        Ok(())
    }

    /// List all events for a session, ordered by sequence number.
    pub fn list_events(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionEventEnvelope>, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");
        let rows: Vec<EventRow> = conn
            .facet_query_ref(
                "SELECT data FROM session_events WHERE session_id = :session_id ORDER BY seq ASC",
                &SessionIdParam {
                    session_id: session_id.to_owned(),
                },
            )
            .map_err(|e| StoreError {
                message: format!("failed to list events for session {session_id}: {e}"),
            })?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let envelope: SessionEventEnvelope =
                facet_json::from_str(&row.data).map_err(|e| StoreError {
                    message: format!(
                        "failed to deserialize event for session {session_id}: {e}"
                    ),
                })?;
            events.push(envelope);
        }
        Ok(events)
    }

    /// Return the count of events stored for a session.
    pub fn event_count(&self, session_id: &str) -> Result<u64, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");
        let row: CountRow = conn
            .facet_query_one_ref(
                "SELECT COUNT(*) AS count FROM session_events WHERE session_id = :session_id",
                &SessionIdParam {
                    session_id: session_id.to_owned(),
                },
            )
            .map_err(|e| StoreError {
                message: format!("failed to count events for session {session_id}: {e}"),
            })?;
        Ok(row.count as u64)
    }

    // ── Activity log ────────────────────────────────────────────────────

    /// Append an activity entry. The `id` field on the input is ignored —
    /// SQLite assigns the auto-increment id. Returns the assigned id.
    pub fn append_activity(&self, entry: &ActivityEntry) -> Result<u64, StoreError> {
        let kind_json = facet_json::to_string_pretty(&entry.kind).map_err(|e| StoreError {
            message: format!("failed to serialize activity kind: {e}"),
        })?;

        let conn = self.conn.lock().expect("session db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT INTO activity_log (timestamp, session_id, session_slug, session_title, kind)
             VALUES (:timestamp, :session_id, :session_slug, :session_title, :kind)",
            &ActivityInsertParams {
                timestamp: entry.timestamp.clone(),
                session_id: entry.session_id.0.clone(),
                session_slug: entry.session_slug.clone(),
                session_title: entry.session_title.clone(),
                kind: kind_json,
            },
        )
        .map_err(|e| StoreError {
            message: format!("failed to append activity: {e}"),
        })?;

        let id: i64 = conn
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
            .map_err(|e| StoreError {
                message: format!("failed to get last insert rowid: {e}"),
            })?;

        Ok(id as u64)
    }

    /// List the most recent activity entries, up to `limit`.
    /// Returns entries in reverse chronological order (newest first).
    pub fn list_activity(&self, limit: u64) -> Result<Vec<ActivityEntry>, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");
        let rows: Vec<ActivityRow> = conn
            .facet_query_ref(
                "SELECT id, timestamp, session_id, session_slug, session_title, kind
                 FROM activity_log ORDER BY id DESC LIMIT :limit",
                &LimitParam {
                    limit: limit as i64,
                },
            )
            .map_err(|e| StoreError {
                message: format!("failed to list activity: {e}"),
            })?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let kind = facet_json::from_str(&row.kind).map_err(|e| StoreError {
                message: format!("failed to deserialize activity kind: {e}"),
            })?;
            entries.push(ActivityEntry {
                id: row.id as u64,
                timestamp: row.timestamp,
                session_id: ship_types::SessionId(row.session_id),
                session_slug: row.session_slug,
                session_title: row.session_title,
                kind,
            });
        }
        Ok(entries)
    }

    /// Trim the activity log to keep only the most recent `keep` entries.
    pub fn trim_activity(&self, keep: u64) -> Result<u64, StoreError> {
        let conn = self.conn.lock().expect("session db mutex poisoned");

        let total: CountRow = conn
            .facet_query_one(
                "SELECT COUNT(*) AS count FROM activity_log",
                (),
            )
            .map_err(|e| StoreError {
                message: format!("failed to count activity log: {e}"),
            })?;

        let to_delete = (total.count as u64).saturating_sub(keep);
        if to_delete == 0 {
            return Ok(0);
        }

        conn.facet_execute_ref(
            "DELETE FROM activity_log WHERE id IN (
                SELECT id FROM activity_log ORDER BY id ASC LIMIT :limit
            )",
            &LimitParam {
                limit: to_delete as i64,
            },
        )
        .map_err(|e| StoreError {
            message: format!("failed to trim activity log: {e}"),
        })?;

        Ok(to_delete)
    }
}

fn workflow_to_str(w: ship_types::Workflow) -> String {
    match w {
        ship_types::Workflow::Merge => "merge".to_owned(),
        ship_types::Workflow::PullRequest => "pull_request".to_owned(),
    }
}

fn deserialize_session(json: &str, display_name: &str) -> Result<PersistedSession, StoreError> {
    facet_json::from_str::<PersistedSession>(json).map_err(|e| StoreError {
        message: format!("failed to deserialize session {display_name}: {e}"),
    })
}

fn collect_sessions<'a>(
    jsons: impl Iterator<Item = &'a str>,
) -> Result<Vec<PersistedSession>, StoreError> {
    let mut sessions = Vec::new();
    for json in jsons {
        match deserialize_session(json, "<list>") {
            Ok(session) => sessions.push(session),
            Err(e) => {
                tracing::warn!("skipping unparseable session: {}", e.message);
            }
        }
    }
    Ok(sessions)
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
                workflow: Default::default(),
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

    fn make_test_event(seq: u64) -> SessionEventEnvelope {
        SessionEventEnvelope {
            seq,
            timestamp: format!("2025-01-01T00:00:{seq:02}Z"),
            event: SessionEvent::AgentStateChanged {
                role: Role::Captain,
                state: AgentState::Working {
                    plan: None,
                    activity: None,
                },
            },
        }
    }

    fn make_test_activity(session_id: &str, kind: ActivityKind) -> ActivityEntry {
        ActivityEntry {
            id: 0,
            timestamp: "2025-01-01T00:00:00Z".to_owned(),
            session_id: SessionId(session_id.to_owned()),
            session_slug: session_id.to_owned(),
            session_title: Some("Test".to_owned()),
            kind,
        }
    }

    // ── Session CRUD tests ──────────────────────────────────────────────

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

    // ── list_sessions_for_project tests ─────────────────────────────────

    #[test]
    fn list_sessions_for_project_filters() {
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

        let a = store.list_sessions_for_project("project-a").unwrap();
        assert_eq!(a.len(), 2);

        let b = store.list_sessions_for_project("project-b").unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].id.0, "s2");

        let c = store.list_sessions_for_project("nonexistent").unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn list_sessions_for_project_excludes_archived() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        store
            .save_session(&make_test_session("s1", "proj"))
            .unwrap();

        let mut archived = make_test_session("s2", "proj");
        archived.archived_at = Some("2025-06-01T00:00:00Z".to_owned());
        store.save_session(&archived).unwrap();

        let result = store.list_sessions_for_project("proj").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.0, "s1");
    }

    // ── Archive / unarchive tests ───────────────────────────────────────

    #[test]
    fn archive_and_unarchive() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        store
            .save_session(&make_test_session("s1", "proj"))
            .unwrap();

        // Archive it
        let found = store
            .archive_session("s1", "2025-06-15T12:00:00Z")
            .unwrap();
        assert!(found);

        // No longer in active list
        assert!(store.list_sessions().unwrap().is_empty());

        // But still loadable, with archived_at set in the JSON blob
        let loaded = store.load_session("s1").unwrap().unwrap();
        assert_eq!(
            loaded.archived_at.as_deref(),
            Some("2025-06-15T12:00:00Z")
        );

        // Unarchive it
        let found = store.unarchive_session("s1").unwrap();
        assert!(found);

        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].archived_at.is_none());
    }

    #[test]
    fn archive_nonexistent_returns_false() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        let found = store
            .archive_session("nope", "2025-06-15T12:00:00Z")
            .unwrap();
        assert!(!found);
    }

    #[test]
    fn unarchive_nonexistent_returns_false() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        let found = store.unarchive_session("nope").unwrap();
        assert!(!found);
    }

    // ── Event append / list tests ───────────────────────────────────────

    #[test]
    fn append_and_list_events() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        store
            .save_session(&make_test_session("s1", "proj"))
            .unwrap();

        store.append_event("s1", &make_test_event(0)).unwrap();
        store.append_event("s1", &make_test_event(1)).unwrap();
        store.append_event("s1", &make_test_event(2)).unwrap();

        let events = store.list_events("s1").unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].seq, 0);
        assert_eq!(events[1].seq, 1);
        assert_eq!(events[2].seq, 2);
    }

    #[test]
    fn event_count() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        store
            .save_session(&make_test_session("s1", "proj"))
            .unwrap();

        assert_eq!(store.event_count("s1").unwrap(), 0);

        store.append_event("s1", &make_test_event(0)).unwrap();
        store.append_event("s1", &make_test_event(1)).unwrap();

        assert_eq!(store.event_count("s1").unwrap(), 2);
    }

    #[test]
    fn list_events_empty_session() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        let events = store.list_events("nonexistent").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn delete_session_cascades_events() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        store
            .save_session(&make_test_session("s1", "proj"))
            .unwrap();

        store.append_event("s1", &make_test_event(0)).unwrap();
        store.append_event("s1", &make_test_event(1)).unwrap();

        store.delete_session("s1").unwrap();

        assert_eq!(store.event_count("s1").unwrap(), 0);
    }

    // ── Activity log tests ──────────────────────────────────────────────

    #[test]
    fn append_and_list_activity() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        let id1 = store
            .append_activity(&make_test_activity("s1", ActivityKind::SessionCreated))
            .unwrap();
        let id2 = store
            .append_activity(&make_test_activity("s1", ActivityKind::CaptainMessage {
                message: "hello".to_owned(),
            }))
            .unwrap();

        assert!(id2 > id1);

        let entries = store.list_activity(10).unwrap();
        assert_eq!(entries.len(), 2);
        // Newest first
        assert_eq!(entries[0].id, id2);
        assert_eq!(entries[1].id, id1);
    }

    #[test]
    fn list_activity_respects_limit() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        for _ in 0..5 {
            store
                .append_activity(&make_test_activity("s1", ActivityKind::SessionCreated))
                .unwrap();
        }

        let entries = store.list_activity(3).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn trim_activity() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        for _ in 0..10 {
            store
                .append_activity(&make_test_activity("s1", ActivityKind::SessionCreated))
                .unwrap();
        }

        let deleted = store.trim_activity(3).unwrap();
        assert_eq!(deleted, 7);

        let remaining = store.list_activity(100).unwrap();
        assert_eq!(remaining.len(), 3);
    }

    #[test]
    fn trim_activity_noop_when_under_limit() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        store
            .append_activity(&make_test_activity("s1", ActivityKind::SessionCreated))
            .unwrap();

        let deleted = store.trim_activity(100).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn activity_kind_roundtrip() {
        let store = SqliteSessionStore::open_in_memory().unwrap();

        let kinds = vec![
            ActivityKind::SessionCreated,
            ActivityKind::SessionArchived,
            ActivityKind::CaptainMessage {
                message: "testing".to_owned(),
            },
            ActivityKind::AdmiralMessage {
                message: "hello".to_owned(),
            },
        ];

        for kind in &kinds {
            store
                .append_activity(&make_test_activity("s1", kind.clone()))
                .unwrap();
        }

        let entries = store.list_activity(10).unwrap();
        assert_eq!(entries.len(), 4);

        // Reversed order (newest first), so compare backwards
        for (i, entry) in entries.iter().rev().enumerate() {
            assert_eq!(entry.kind, kinds[i]);
        }
    }
}
