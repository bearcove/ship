use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use rusqlite_facet::{ConnectionFacetExt, StatementFacetExt};
use ship_policy::{AgentRole, Block, BlockContent, BlockId, ParticipantName, Participant, RoomId, Lane, Task, TaskId, TaskPhase, Topology};

use crate::schema;
use ship_types::{ActivityEntry, PersistedSession, ProjectName, SessionEventEnvelope, SessionId};

/// Error type for database operations.
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
struct IdParam<'a> {
    id: &'a str,
}

#[derive(Debug, facet::Facet)]
struct InsertParams<'a> {
    id: &'a SessionId,
    created_at: &'a str,
    archived_at: Option<&'a str>,
    title: Option<&'a str>,
    is_read: i64,
    project: &'a ProjectName,
    base_branch: &'a str,
    branch_name: &'a str,
    workflow: &'a str,
    data: &'a str,
}

#[derive(Debug, facet::Facet)]
struct ListRow {
    data: String,
    archived_at: Option<String>,
}

#[derive(Debug, facet::Facet)]
struct ProjectParam<'a> {
    project: &'a str,
}

#[derive(Debug, facet::Facet)]
struct ArchiveParams<'a> {
    id: &'a str,
    archived_at: Option<&'a str>,
    data: &'a str,
}

#[derive(Debug, facet::Facet)]
struct EventInsertParams<'a> {
    session_id: &'a str,
    seq: i64,
    timestamp: &'a str,
    data: &'a str,
}

#[derive(Debug, facet::Facet)]
struct SessionIdParam<'a> {
    session_id: &'a str,
}

#[derive(Debug, facet::Facet)]
struct EventRow {
    data: String,
}

#[derive(Debug, facet::Facet)]
struct ActivityInsertParams<'a> {
    timestamp: &'a str,
    session_id: &'a SessionId,
    session_slug: &'a str,
    session_title: Option<&'a str>,
    kind: &'a str,
}

#[derive(Debug, facet::Facet)]
struct ActivityRow {
    id: i64,
    timestamp: String,
    session_id: SessionId,
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

// ── Block row/param types ──────────────────────────────────────────────

#[derive(Debug, facet::Facet)]
struct BlockInsertParams<'a> {
    id: &'a BlockId,
    room_id: &'a RoomId,
    seq: i64,
    from_participant: Option<&'a ParticipantName>,
    to_participant: Option<&'a ParticipantName>,
    created_at: jiff::Timestamp,
    sealed_at: Option<jiff::Timestamp>,
    content: &'a str,
}

#[derive(Debug, facet::Facet)]
struct BlockRow {
    id: BlockId,
    room_id: RoomId,
    seq: i64,
    from_participant: Option<ParticipantName>,
    to_participant: Option<ParticipantName>,
    created_at: jiff::Timestamp,
    sealed_at: Option<jiff::Timestamp>,
    content: String,
}

#[derive(Debug, facet::Facet)]
struct BlockRoomParams<'a> {
    room_id: &'a RoomId,
}

#[derive(Debug, facet::Facet)]
struct BlockSealParams<'a> {
    id: &'a BlockId,
    sealed_at: jiff::Timestamp,
    content: &'a str,
}

#[derive(Debug, facet::Facet)]
struct BlockUpdateContentParams<'a> {
    id: &'a BlockId,
    content: &'a str,
}

// ── Task row/param types ──────────────────────────────────────────────

#[derive(Debug, facet::Facet)]
struct TaskInsertParams<'a> {
    id: &'a TaskId,
    room_id: &'a RoomId,
    title: &'a str,
    description: &'a str,
    phase: &'a str,
    created_at: jiff::Timestamp,
    completed_at: Option<jiff::Timestamp>,
}

#[derive(Debug, facet::Facet)]
struct TaskRow {
    id: TaskId,
    room_id: RoomId,
    title: String,
    description: String,
    phase: String,
    created_at: jiff::Timestamp,
    completed_at: Option<jiff::Timestamp>,
}

#[derive(Debug, facet::Facet)]
struct TaskPhaseUpdateParams<'a> {
    id: &'a TaskId,
    phase: &'a str,
    completed_at: Option<jiff::Timestamp>,
}

#[derive(Debug, facet::Facet)]
struct TaskRoomParams<'a> {
    room_id: &'a RoomId,
}

#[derive(Debug, facet::Facet)]
struct CurrentTaskParams<'a> {
    current_task_id: Option<&'a TaskId>,
    id: &'a RoomId,
}

/// SQLite-backed persistence for all of ship's data.
///
/// Thread-safe via internal Mutex on the connection. All public methods
/// are synchronous — callers should use `spawn_blocking` from async code.
pub struct ShipDb {
    conn: Arc<Mutex<Connection>>,
}

impl ShipDb {
    /// Open (or create) the database at the given path.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError {
            message: format!("failed to open database: {e}"),
        })?;
        schema::init(&conn).map_err(|e| StoreError {
            message: format!("failed to initialize schema: {e}"),
        })?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory database (for testing).
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

        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT OR REPLACE INTO sessions (id, created_at, archived_at, title, is_read, project, base_branch, branch_name, workflow, data)
             VALUES (:id, :created_at, :archived_at, :title, :is_read, :project, :base_branch, :branch_name, :workflow, :data)",
            &InsertParams {
                id: &session.id,
                created_at: &session.created_at,
                archived_at: session.archived_at.as_deref(),
                title: session.title.as_deref(),
                is_read: if session.is_read { 1 } else { 0 },
                project: &session.config.project,
                base_branch: &session.config.base_branch,
                branch_name: &session.config.branch_name,
                workflow: workflow_to_str(session.config.workflow),
                data: &data,
            },
        )
        .map_err(|e| StoreError {
            message: format!("failed to save session {}: {e}", session.id.0),
        })?;

        Ok(())
    }

    /// Load a single session by ID.
    pub fn load_session(&self, id: &str) -> Result<Option<PersistedSession>, StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let row: Option<SessionRow> = conn
            .facet_query_optional_ref(
                "SELECT data FROM sessions WHERE id = :id",
                &IdParam { id },
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
        let conn = self.conn.lock().expect("db mutex poisoned");
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
        let conn = self.conn.lock().expect("db mutex poisoned");
        let rows: Vec<ListRow> = conn
            .facet_query_ref(
                "SELECT data, archived_at FROM sessions WHERE archived_at IS NULL AND project = :project",
                &ProjectParam {
                    project,
                },
            )
            .map_err(|e| StoreError {
                message: format!("failed to list sessions for project {project}: {e}"),
            })?;

        collect_sessions(rows.iter().map(|r| r.data.as_str()))
    }

    /// Delete a session by ID. Also deletes associated events (ON DELETE CASCADE).
    pub fn delete_session(&self, id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "DELETE FROM sessions WHERE id = :id",
            &IdParam { id },
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
        let conn = self.conn.lock().expect("db mutex poisoned");

        let row: Option<SessionRow> = conn
            .facet_query_optional_ref(
                "SELECT data FROM sessions WHERE id = :id",
                &IdParam { id },
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
                id,
                archived_at: Some(timestamp),
                data: &data,
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
        let conn = self.conn.lock().expect("db mutex poisoned");

        let row: Option<SessionRow> = conn
            .facet_query_optional_ref(
                "SELECT data FROM sessions WHERE id = :id",
                &IdParam { id },
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
                id,
                archived_at: None,
                data: &data,
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

        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT INTO session_events (session_id, seq, timestamp, data)
             VALUES (:session_id, :seq, :timestamp, :data)",
            &EventInsertParams {
                session_id,
                seq: envelope.seq as i64,
                timestamp: &envelope.timestamp,
                data: &data,
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
        let conn = self.conn.lock().expect("db mutex poisoned");
        let rows: Vec<EventRow> = conn
            .facet_query_ref(
                "SELECT data FROM session_events WHERE session_id = :session_id ORDER BY seq ASC",
                &SessionIdParam { session_id },
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
        let conn = self.conn.lock().expect("db mutex poisoned");
        let row: CountRow = conn
            .facet_query_one_ref(
                "SELECT COUNT(*) AS count FROM session_events WHERE session_id = :session_id",
                &SessionIdParam { session_id },
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

        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT INTO activity_log (timestamp, session_id, session_slug, session_title, kind)
             VALUES (:timestamp, :session_id, :session_slug, :session_title, :kind)",
            &ActivityInsertParams {
                timestamp: &entry.timestamp,
                session_id: &entry.session_id,
                session_slug: &entry.session_slug,
                session_title: entry.session_title.as_deref(),
                kind: &kind_json,
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
        let conn = self.conn.lock().expect("db mutex poisoned");
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
                session_id: row.session_id,
                session_slug: row.session_slug,
                session_title: row.session_title,
                kind,
            });
        }
        Ok(entries)
    }

    /// Trim the activity log to keep only the most recent `keep` entries.
    pub fn trim_activity(&self, keep: u64) -> Result<u64, StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");

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

    // ── Blocks ──────────────────────────────────────────────────────────

    /// Insert a block into the database.
    pub fn insert_block(&self, block: &Block) -> Result<(), StoreError> {
        let content = facet_json::to_string(&block.content).map_err(|e| StoreError {
            message: format!("failed to serialize block content: {e}"),
        })?;
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT INTO blocks (id, room_id, seq, from_participant, to_participant, created_at, sealed_at, content)
             VALUES (:id, :room_id, :seq, :from_participant, :to_participant, :created_at, :sealed_at, :content)",
            &BlockInsertParams {
                id: &block.id,
                room_id: &block.room_id,
                seq: block.seq as i64,
                from_participant: block.from.as_ref(),
                to_participant: block.to.as_ref(),
                created_at: block.created_at,
                sealed_at: block.sealed_at,
                content: &content,
            },
        )
        .map_err(fe)?;
        Ok(())
    }

    /// Load all blocks for a room, ordered by seq.
    pub fn list_blocks(&self, room_id: &RoomId) -> Result<Vec<Block>, StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT id, room_id, seq, from_participant, to_participant, created_at, sealed_at, content FROM blocks WHERE room_id = :room_id ORDER BY seq")
            .map_err(se)?;
        let rows: Vec<BlockRow> = stmt
            .facet_query_ref(&BlockRoomParams { room_id })
            .map_err(fe)?;

        rows.into_iter().map(block_from_row).collect()
    }

    /// Seal a block: set sealed_at and update content.
    pub fn seal_block(
        &self,
        id: &BlockId,
        sealed_at: jiff::Timestamp,
        content: &BlockContent,
    ) -> Result<(), StoreError> {
        let content_json = facet_json::to_string(content).map_err(|e| StoreError {
            message: format!("failed to serialize block content: {e}"),
        })?;
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "UPDATE blocks SET sealed_at = :sealed_at, content = :content WHERE id = :id",
            &BlockSealParams {
                id,
                sealed_at,
                content: &content_json,
            },
        )
        .map_err(fe)?;
        Ok(())
    }

    /// Update an unsealed block's content (e.g. text append during streaming).
    pub fn update_block_content(
        &self,
        id: &BlockId,
        content: &BlockContent,
    ) -> Result<(), StoreError> {
        let content_json = facet_json::to_string(content).map_err(|e| StoreError {
            message: format!("failed to serialize block content: {e}"),
        })?;
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "UPDATE blocks SET content = :content WHERE id = :id AND sealed_at IS NULL",
            &BlockUpdateContentParams {
                id,
                content: &content_json,
            },
        )
        .map_err(fe)?;
        Ok(())
    }

    // ── Tasks ───────────────────────────────────────────────────────────

    /// Insert a new task.
    pub fn insert_task(&self, task: &Task) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "INSERT INTO tasks (id, room_id, title, description, phase, created_at, completed_at)
             VALUES (:id, :room_id, :title, :description, :phase, :created_at, :completed_at)",
            &TaskInsertParams {
                id: &task.id,
                room_id: &task.room_id,
                title: &task.title,
                description: &task.description,
                phase: phase_to_str(task.phase),
                created_at: task.created_at,
                completed_at: task.completed_at,
            },
        )
        .map_err(fe)?;
        Ok(())
    }

    /// Load a task by id.
    pub fn load_task(&self, id: &TaskId) -> Result<Option<Task>, StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT id, room_id, title, description, phase, created_at, completed_at FROM tasks WHERE id = :id")
            .map_err(se)?;
        let row: Option<TaskRow> = stmt
            .facet_query_optional_ref(&BlockIdParam { id: id.as_str() })
            .map_err(fe)?;
        row.map(task_from_row).transpose()
    }

    /// Load the current task for a room (if any).
    pub fn current_task(&self, room_id: &RoomId) -> Result<Option<Task>, StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.room_id, t.title, t.description, t.phase, t.created_at, t.completed_at
                 FROM tasks t
                 JOIN rooms r ON r.current_task_id = t.id
                 WHERE r.id = :room_id",
            )
            .map_err(se)?;
        let row: Option<TaskRow> = stmt
            .facet_query_optional_ref(&TaskRoomParams { room_id })
            .map_err(fe)?;
        row.map(task_from_row).transpose()
    }

    /// List all tasks for a room, ordered by creation time.
    pub fn list_tasks(&self, room_id: &RoomId) -> Result<Vec<Task>, StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT id, room_id, title, description, phase, created_at, completed_at FROM tasks WHERE room_id = :room_id ORDER BY created_at")
            .map_err(se)?;
        let rows: Vec<TaskRow> = stmt
            .facet_query_ref(&TaskRoomParams { room_id })
            .map_err(fe)?;
        rows.into_iter().map(task_from_row).collect()
    }

    /// Update a task's phase. Sets completed_at if the phase is terminal.
    pub fn update_task_phase(
        &self,
        id: &TaskId,
        phase: TaskPhase,
        completed_at: Option<jiff::Timestamp>,
    ) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "UPDATE tasks SET phase = :phase, completed_at = :completed_at WHERE id = :id",
            &TaskPhaseUpdateParams {
                id,
                phase: phase_to_str(phase),
                completed_at,
            },
        )
        .map_err(fe)?;
        Ok(())
    }

    /// Set the current task for a room. Pass None to clear.
    pub fn set_current_task(
        &self,
        room_id: &RoomId,
        task_id: Option<&TaskId>,
    ) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.facet_execute_ref(
            "UPDATE rooms SET current_task_id = :current_task_id WHERE id = :id",
            &CurrentTaskParams {
                current_task_id: task_id,
                id: room_id,
            },
        )
        .map_err(fe)?;
        Ok(())
    }

    // ── Topology ────────────────────────────────────────────────────────

    /// Persist a full topology, replacing whatever was there before.
    /// Runs in a single transaction.
    pub fn save_topology(&self, topology: &Topology) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute_batch("BEGIN").map_err(se)?;

        // Clear existing topology
        conn.execute_batch(
            "DELETE FROM memberships; DELETE FROM rooms; DELETE FROM participants;",
        )
        .map_err(se)?;

        // Insert participants
        let mut insert_participant = conn
            .prepare_cached(
                "INSERT INTO participants (name, kind) VALUES (?1, ?2)",
            )
            .map_err(se)?;

        insert_participant
            .execute(rusqlite::params![topology.human.name, "human"])
            .map_err(se)?;
        insert_participant
            .execute(rusqlite::params![topology.admiral.name, "admiral"])
            .map_err(se)?;

        for session in &topology.lanes {
            insert_participant
                .execute(rusqlite::params![session.captain.name, "captain"])
                .map_err(se)?;
            insert_participant
                .execute(rusqlite::params![session.mate.name, "mate"])
                .map_err(se)?;
        }

        // Insert rooms and memberships
        let mut insert_room = conn
            .prepare_cached(
                "INSERT INTO rooms (id, kind, session_id) VALUES (?1, ?2, ?3)",
            )
            .map_err(se)?;
        let mut insert_membership = conn
            .prepare_cached(
                "INSERT INTO memberships (room_id, participant_name) VALUES (?1, ?2)",
            )
            .map_err(se)?;

        // Admiral room: admiral + all captains
        insert_room
            .execute(rusqlite::params!["admiral", "admiral", Option::<&str>::None])
            .map_err(se)?;
        insert_membership
            .execute(rusqlite::params!["admiral", topology.admiral.name])
            .map_err(se)?;
        for session in &topology.lanes {
            insert_membership
                .execute(rusqlite::params!["admiral", session.captain.name])
                .map_err(se)?;
        }

        // Session rooms: captain + mate
        for session in &topology.lanes {
            let room_id = session.id.as_str();
            let session_id = room_id.strip_prefix("session:");
            insert_room
                .execute(rusqlite::params![room_id, "session", session_id])
                .map_err(se)?;
            insert_membership
                .execute(rusqlite::params![room_id, session.captain.name])
                .map_err(se)?;
            insert_membership
                .execute(rusqlite::params![room_id, session.mate.name])
                .map_err(se)?;
        }

        conn.execute_batch("COMMIT").map_err(se)?;
        Ok(())
    }

    /// Load the full topology from the database.
    /// Returns None if no participants exist (empty topology).
    pub fn load_topology(&self) -> Result<Option<Topology>, StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");

        // Load all participants
        let mut stmt = conn
            .prepare("SELECT name, kind FROM participants")
            .map_err(se)?;
        let participant_rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(se)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(se)?;

        if participant_rows.is_empty() {
            return Ok(None);
        }

        // Find the human and admiral
        let human = participant_rows
            .iter()
            .find(|(_, k)| k == "human")
            .map(|(name, _)| Participant::human(name.clone()))
            .ok_or_else(|| StoreError {
                message: "topology has no human participant".into(),
            })?;

        let admiral = participant_rows
            .iter()
            .find(|(_, k)| k == "admiral")
            .map(|(name, _)| Participant::agent(name.clone(), AgentRole::Admiral))
            .ok_or_else(|| StoreError {
                message: "topology has no admiral participant".into(),
            })?;

        // Load session rooms
        let mut room_stmt = conn
            .prepare("SELECT id, kind, session_id FROM rooms WHERE kind = 'session'")
            .map_err(se)?;
        let session_rooms: Vec<(String, Option<String>)> = room_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(2)?)))
            .map_err(se)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(se)?;

        // Load memberships for each session room
        let mut membership_stmt = conn
            .prepare("SELECT participant_name FROM memberships WHERE room_id = ?1")
            .map_err(se)?;

        let find_participant = |name: &str, expected_kind: &str| -> Result<Participant, StoreError> {
            participant_rows
                .iter()
                .find(|(n, k)| n == name && k == expected_kind)
                .map(|(n, k)| to_participant(n, k))
                .ok_or_else(|| StoreError {
                    message: format!("expected {expected_kind} participant '{name}' not found"),
                })
        };

        let mut sessions = Vec::new();
        for (room_id, _session_id) in &session_rooms {
            let members: Vec<String> = membership_stmt
                .query_map([room_id], |row| row.get(0))
                .map_err(se)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(se)?;

            let captain_name = members
                .iter()
                .find(|name| participant_rows.iter().any(|(n, k)| n == *name && k == "captain"))
                .ok_or_else(|| StoreError {
                    message: format!("session room '{room_id}' has no captain"),
                })?;

            let mate_name = members
                .iter()
                .find(|name| participant_rows.iter().any(|(n, k)| n == *name && k == "mate"))
                .ok_or_else(|| StoreError {
                    message: format!("session room '{room_id}' has no mate"),
                })?;

            sessions.push(Lane {
                id: RoomId::from(room_id.clone()),
                captain: find_participant(captain_name, "captain")?,
                mate: find_participant(mate_name, "mate")?,
            });
        }

        Ok(Some(Topology {
            human,
            admiral,
            lanes: sessions,
        }))
    }

    /// Add a single session room to the existing topology.
    /// Inserts the captain + mate as participants, creates the room, and wires up memberships.
    pub fn add_lane(&self, session: &Lane) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute_batch("BEGIN").map_err(se)?;

        conn.execute(
            "INSERT OR IGNORE INTO participants (name, kind) VALUES (?1, 'captain')",
            [&session.captain.name],
        )
        .map_err(se)?;
        conn.execute(
            "INSERT OR IGNORE INTO participants (name, kind) VALUES (?1, 'mate')",
            [&session.mate.name],
        )
        .map_err(se)?;

        let room_id = session.id.as_str();
        let session_id = room_id.strip_prefix("session:");
        conn.execute(
            "INSERT INTO rooms (id, kind, session_id) VALUES (?1, 'session', ?2)",
            rusqlite::params![room_id, session_id],
        )
        .map_err(se)?;

        conn.execute(
            "INSERT INTO memberships (room_id, participant_name) VALUES (?1, ?2)",
            rusqlite::params![room_id, session.captain.name],
        )
        .map_err(se)?;
        conn.execute(
            "INSERT INTO memberships (room_id, participant_name) VALUES (?1, ?2)",
            rusqlite::params![room_id, session.mate.name],
        )
        .map_err(se)?;

        conn.execute(
            "INSERT OR IGNORE INTO memberships (room_id, participant_name) VALUES ('admiral', ?1)",
            [&session.captain.name],
        )
        .map_err(se)?;

        conn.execute_batch("COMMIT").map_err(se)?;
        Ok(())
    }

    /// Remove a session room and its participants from the topology.
    /// Cascading deletes handle memberships.
    pub fn remove_lane(&self, room_id: &RoomId) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute_batch("BEGIN").map_err(se)?;

        let mut stmt = conn
            .prepare("SELECT participant_name FROM memberships WHERE room_id = ?1")
            .map_err(se)?;
        let members: Vec<String> = stmt
            .query_map([room_id.as_str()], |row| row.get(0))
            .map_err(se)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(se)?;

        conn.execute("DELETE FROM rooms WHERE id = ?1", [room_id.as_str()])
            .map_err(se)?;

        for name in &members {
            let in_any_room: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM memberships WHERE participant_name = ?1",
                    [name],
                    |row| row.get(0),
                )
                .map_err(se)?;
            if !in_any_room {
                conn.execute("DELETE FROM participants WHERE name = ?1", [name])
                    .map_err(se)?;
            }
        }

        conn.execute_batch("COMMIT").map_err(se)?;
        Ok(())
    }
}

fn to_participant(name: &str, kind: &str) -> Participant {
    match kind {
        "human" => Participant::human(name),
        "admiral" => Participant::agent(name, AgentRole::Admiral),
        "captain" => Participant::agent(name, AgentRole::Captain),
        "mate" => Participant::agent(name, AgentRole::Mate),
        _ => unreachable!("CHECK constraint prevents invalid kind: {kind}"),
    }
}

fn block_from_row(row: BlockRow) -> Result<Block, StoreError> {
    let content: BlockContent = facet_json::from_str(&row.content).map_err(|e| StoreError {
        message: format!("failed to deserialize block content for {}: {e}", row.id),
    })?;
    Ok(Block {
        id: row.id,
        room_id: row.room_id,
        seq: row.seq as u64,
        from: row.from_participant,
        to: row.to_participant,
        created_at: row.created_at,
        sealed_at: row.sealed_at,
        content,
    })
}

fn se(e: rusqlite::Error) -> StoreError {
    StoreError {
        message: e.to_string(),
    }
}

fn task_from_row(row: TaskRow) -> Result<Task, StoreError> {
    let phase = phase_from_str(&row.phase).ok_or_else(|| StoreError {
        message: format!("unknown task phase '{}' for task {}", row.phase, row.id),
    })?;
    Ok(Task {
        id: row.id,
        room_id: row.room_id,
        title: row.title,
        description: row.description,
        phase,
        created_at: row.created_at,
        completed_at: row.completed_at,
    })
}

fn phase_to_str(phase: TaskPhase) -> &'static str {
    match phase {
        TaskPhase::Assigned => "assigned",
        TaskPhase::Working => "working",
        TaskPhase::PendingReview => "pending_review",
        TaskPhase::RebaseConflict => "rebase_conflict",
        TaskPhase::Accepted => "accepted",
        TaskPhase::Cancelled => "cancelled",
    }
}

fn phase_from_str(s: &str) -> Option<TaskPhase> {
    match s {
        "assigned" => Some(TaskPhase::Assigned),
        "working" => Some(TaskPhase::Working),
        "pending_review" => Some(TaskPhase::PendingReview),
        "rebase_conflict" => Some(TaskPhase::RebaseConflict),
        "accepted" => Some(TaskPhase::Accepted),
        "cancelled" => Some(TaskPhase::Cancelled),
        _ => None,
    }
}

#[derive(Debug, facet::Facet)]
struct BlockIdParam<'a> {
    id: &'a str,
}

fn fe(e: rusqlite_facet::Error) -> StoreError {
    StoreError {
        message: e.to_string(),
    }
}

fn workflow_to_str(w: ship_types::Workflow) -> &'static str {
    match w {
        ship_types::Workflow::Merge => "merge",
        ship_types::Workflow::PullRequest => "pull_request",
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

    fn sample_topology(db: &ShipDb) -> Topology {
        // Create stub sessions so FK constraints on rooms.session_id are satisfied
        db.save_session(&make_test_session("s1", "proj")).unwrap();
        db.save_session(&make_test_session("s2", "proj")).unwrap();
        Topology {
            human: Participant::human("Amos"),
            admiral: Participant::agent("Admiral", AgentRole::Admiral),
            lanes: vec![
                Lane {
                    id: RoomId::from_static("session:s1"),
                    captain: Participant::agent("Alex", AgentRole::Captain),
                    mate: Participant::agent("Jordan", AgentRole::Mate),
                },
                Lane {
                    id: RoomId::from_static("session:s2"),
                    captain: Participant::agent("Morgan", AgentRole::Captain),
                    mate: Participant::agent("Riley", AgentRole::Mate),
                },
            ],
        }
    }

    // ── Session CRUD tests ──────────────────────────────────────────────

    #[test]
    fn save_and_load_roundtrip() {
        let db = ShipDb::open_in_memory().unwrap();
        let session = make_test_session("sess-001", "myproject");

        db.save_session(&session).unwrap();

        let loaded = db.load_session("sess-001").unwrap().unwrap();
        assert_eq!(loaded.id.0, "sess-001");
        assert_eq!(loaded.config.project.0, "myproject");
        assert_eq!(loaded.title.as_deref(), Some("Test session"));
        assert!(!loaded.is_read);
    }

    #[test]
    fn load_missing_returns_none() {
        let db = ShipDb::open_in_memory().unwrap();
        let loaded = db.load_session("nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn list_excludes_archived() {
        let db = ShipDb::open_in_memory().unwrap();

        let active = make_test_session("sess-active", "proj");
        db.save_session(&active).unwrap();

        let mut archived = make_test_session("sess-archived", "proj");
        archived.archived_at = Some("2025-06-01T00:00:00Z".to_owned());
        db.save_session(&archived).unwrap();

        let sessions = db.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id.0, "sess-active");
    }

    #[test]
    fn save_upserts() {
        let db = ShipDb::open_in_memory().unwrap();

        let mut session = make_test_session("sess-001", "proj");
        db.save_session(&session).unwrap();

        session.title = Some("Updated title".to_owned());
        session.is_read = true;
        db.save_session(&session).unwrap();

        let loaded = db.load_session("sess-001").unwrap().unwrap();
        assert_eq!(loaded.title.as_deref(), Some("Updated title"));
        assert!(loaded.is_read);

        let all = db.list_sessions().unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn delete_session() {
        let db = ShipDb::open_in_memory().unwrap();

        let session = make_test_session("sess-001", "proj");
        db.save_session(&session).unwrap();

        db.delete_session("sess-001").unwrap();

        let loaded = db.load_session("sess-001").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        let db = ShipDb::open_in_memory().unwrap();
        db.delete_session("nonexistent").unwrap();
    }

    #[test]
    fn list_multiple_projects() {
        let db = ShipDb::open_in_memory().unwrap();

        db.save_session(&make_test_session("s1", "project-a")).unwrap();
        db.save_session(&make_test_session("s2", "project-b")).unwrap();
        db.save_session(&make_test_session("s3", "project-a")).unwrap();

        let all = db.list_sessions().unwrap();
        assert_eq!(all.len(), 3);
    }

    // ── list_sessions_for_project tests ─────────────────────────────────

    #[test]
    fn list_sessions_for_project_filters() {
        let db = ShipDb::open_in_memory().unwrap();

        db.save_session(&make_test_session("s1", "project-a")).unwrap();
        db.save_session(&make_test_session("s2", "project-b")).unwrap();
        db.save_session(&make_test_session("s3", "project-a")).unwrap();

        let a = db.list_sessions_for_project("project-a").unwrap();
        assert_eq!(a.len(), 2);

        let b = db.list_sessions_for_project("project-b").unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].id.0, "s2");

        let c = db.list_sessions_for_project("nonexistent").unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn list_sessions_for_project_excludes_archived() {
        let db = ShipDb::open_in_memory().unwrap();

        db.save_session(&make_test_session("s1", "proj")).unwrap();

        let mut archived = make_test_session("s2", "proj");
        archived.archived_at = Some("2025-06-01T00:00:00Z".to_owned());
        db.save_session(&archived).unwrap();

        let result = db.list_sessions_for_project("proj").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.0, "s1");
    }

    // ── Archive / unarchive tests ───────────────────────────────────────

    #[test]
    fn archive_and_unarchive() {
        let db = ShipDb::open_in_memory().unwrap();
        db.save_session(&make_test_session("s1", "proj")).unwrap();

        let found = db.archive_session("s1", "2025-06-15T12:00:00Z").unwrap();
        assert!(found);

        assert!(db.list_sessions().unwrap().is_empty());

        let loaded = db.load_session("s1").unwrap().unwrap();
        assert_eq!(loaded.archived_at.as_deref(), Some("2025-06-15T12:00:00Z"));

        let found = db.unarchive_session("s1").unwrap();
        assert!(found);

        let sessions = db.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].archived_at.is_none());
    }

    #[test]
    fn archive_nonexistent_returns_false() {
        let db = ShipDb::open_in_memory().unwrap();
        let found = db.archive_session("nope", "2025-06-15T12:00:00Z").unwrap();
        assert!(!found);
    }

    #[test]
    fn unarchive_nonexistent_returns_false() {
        let db = ShipDb::open_in_memory().unwrap();
        let found = db.unarchive_session("nope").unwrap();
        assert!(!found);
    }

    // ── Event append / list tests ───────────────────────────────────────

    #[test]
    fn append_and_list_events() {
        let db = ShipDb::open_in_memory().unwrap();
        db.save_session(&make_test_session("s1", "proj")).unwrap();

        db.append_event("s1", &make_test_event(0)).unwrap();
        db.append_event("s1", &make_test_event(1)).unwrap();
        db.append_event("s1", &make_test_event(2)).unwrap();

        let events = db.list_events("s1").unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].seq, 0);
        assert_eq!(events[1].seq, 1);
        assert_eq!(events[2].seq, 2);
    }

    #[test]
    fn event_count() {
        let db = ShipDb::open_in_memory().unwrap();
        db.save_session(&make_test_session("s1", "proj")).unwrap();

        assert_eq!(db.event_count("s1").unwrap(), 0);

        db.append_event("s1", &make_test_event(0)).unwrap();
        db.append_event("s1", &make_test_event(1)).unwrap();

        assert_eq!(db.event_count("s1").unwrap(), 2);
    }

    #[test]
    fn list_events_empty_session() {
        let db = ShipDb::open_in_memory().unwrap();
        let events = db.list_events("nonexistent").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn delete_session_cascades_events() {
        let db = ShipDb::open_in_memory().unwrap();
        db.save_session(&make_test_session("s1", "proj")).unwrap();

        db.append_event("s1", &make_test_event(0)).unwrap();
        db.append_event("s1", &make_test_event(1)).unwrap();

        db.delete_session("s1").unwrap();

        assert_eq!(db.event_count("s1").unwrap(), 0);
    }

    // ── Activity log tests ──────────────────────────────────────────────

    #[test]
    fn append_and_list_activity() {
        let db = ShipDb::open_in_memory().unwrap();

        let id1 = db
            .append_activity(&make_test_activity("s1", ActivityKind::SessionCreated))
            .unwrap();
        let id2 = db
            .append_activity(&make_test_activity("s1", ActivityKind::CaptainMessage {
                message: "hello".to_owned(),
            }))
            .unwrap();

        assert!(id2 > id1);

        let entries = db.list_activity(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, id2);
        assert_eq!(entries[1].id, id1);
    }

    #[test]
    fn list_activity_respects_limit() {
        let db = ShipDb::open_in_memory().unwrap();

        for _ in 0..5 {
            db.append_activity(&make_test_activity("s1", ActivityKind::SessionCreated))
                .unwrap();
        }

        let entries = db.list_activity(3).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn trim_activity() {
        let db = ShipDb::open_in_memory().unwrap();

        for _ in 0..10 {
            db.append_activity(&make_test_activity("s1", ActivityKind::SessionCreated))
                .unwrap();
        }

        let deleted = db.trim_activity(3).unwrap();
        assert_eq!(deleted, 7);

        let remaining = db.list_activity(100).unwrap();
        assert_eq!(remaining.len(), 3);
    }

    #[test]
    fn trim_activity_noop_when_under_limit() {
        let db = ShipDb::open_in_memory().unwrap();

        db.append_activity(&make_test_activity("s1", ActivityKind::SessionCreated))
            .unwrap();

        let deleted = db.trim_activity(100).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn activity_kind_roundtrip() {
        let db = ShipDb::open_in_memory().unwrap();

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
            db.append_activity(&make_test_activity("s1", kind.clone()))
                .unwrap();
        }

        let entries = db.list_activity(10).unwrap();
        assert_eq!(entries.len(), 4);

        for (i, entry) in entries.iter().rev().enumerate() {
            assert_eq!(entry.kind, kinds[i]);
        }
    }

    // ── Topology tests ──────────────────────────────────────────────────

    #[test]
    fn topology_save_and_load_roundtrip() {
        let db = ShipDb::open_in_memory().unwrap();
        let topo = sample_topology(&db);

        db.save_topology(&topo).unwrap();
        let loaded = db.load_topology().unwrap().unwrap();

        assert_eq!(loaded.human, topo.human);
        assert_eq!(loaded.admiral, topo.admiral);
        assert_eq!(loaded.lanes.len(), 2);

        for expected in &topo.lanes {
            let found = loaded
                .lanes
                .iter()
                .find(|s| s.id == expected.id)
                .expect("session room not found");
            assert_eq!(found.captain, expected.captain);
            assert_eq!(found.mate, expected.mate);
        }
    }

    #[test]
    fn topology_load_empty_returns_none() {
        let db = ShipDb::open_in_memory().unwrap();
        assert!(db.load_topology().unwrap().is_none());
    }

    #[test]
    fn topology_save_replaces_previous() {
        let db = ShipDb::open_in_memory().unwrap();

        let topo1 = sample_topology(&db);
        db.save_topology(&topo1).unwrap();

        let topo2 = Topology {
            human: Participant::human("Bob"),
            admiral: Participant::agent("Fleet", AgentRole::Admiral),
            lanes: vec![],
        };
        db.save_topology(&topo2).unwrap();

        let loaded = db.load_topology().unwrap().unwrap();
        assert_eq!(loaded.human.name, "Bob");
        assert_eq!(loaded.admiral.name, "Fleet");
        assert!(loaded.lanes.is_empty());
    }

    #[test]
    fn topology_add_lane() {
        let db = ShipDb::open_in_memory().unwrap();

        let topo = Topology {
            human: Participant::human("Amos"),
            admiral: Participant::agent("Admiral", AgentRole::Admiral),
            lanes: vec![],
        };
        db.save_topology(&topo).unwrap();

        db.save_session(&make_test_session("s1", "proj")).unwrap();
        let session = Lane {
            id: RoomId::from_static("session:s1"),
            captain: Participant::agent("Alex", AgentRole::Captain),
            mate: Participant::agent("Jordan", AgentRole::Mate),
        };
        db.add_lane(&session).unwrap();

        let loaded = db.load_topology().unwrap().unwrap();
        assert_eq!(loaded.lanes.len(), 1);
        assert_eq!(loaded.lanes[0].captain.name, "Alex");
        assert_eq!(loaded.lanes[0].mate.name, "Jordan");

        let admiral_members = loaded.admiral_room_members();
        assert!(admiral_members.iter().any(|p| p.name == "Alex"));
    }

    #[test]
    fn topology_remove_lane() {
        let db = ShipDb::open_in_memory().unwrap();
        let topo = sample_topology(&db);
        db.save_topology(&topo).unwrap();

        db.remove_lane(&RoomId::from_static("session:s1")).unwrap();

        let loaded = db.load_topology().unwrap().unwrap();
        assert_eq!(loaded.lanes.len(), 1);
        assert_eq!(loaded.lanes[0].id, RoomId::from_static("session:s2"));

        let all_names: Vec<&str> = loaded
            .admiral_room_members()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert!(!all_names.contains(&"Alex"));
    }

    #[test]
    fn topology_add_multiple_session_rooms_incrementally() {
        let db = ShipDb::open_in_memory().unwrap();

        let topo = Topology {
            human: Participant::human("Amos"),
            admiral: Participant::agent("Admiral", AgentRole::Admiral),
            lanes: vec![],
        };
        db.save_topology(&topo).unwrap();

        for i in 0..3 {
            db.save_session(&make_test_session(&format!("s{i}"), "proj")).unwrap();
            db.add_lane(&Lane {
                id: RoomId::new(format!("session:s{i}")),
                captain: Participant::agent(format!("Captain{i}"), AgentRole::Captain),
                mate: Participant::agent(format!("Mate{i}"), AgentRole::Mate),
            })
            .unwrap();
        }

        let loaded = db.load_topology().unwrap().unwrap();
        assert_eq!(loaded.lanes.len(), 3);

        let admiral_members = loaded.admiral_room_members();
        assert_eq!(admiral_members.len(), 4); // admiral + 3 captains
    }
}
