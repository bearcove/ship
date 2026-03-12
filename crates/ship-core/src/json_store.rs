use std::io;
use std::path::PathBuf;

use fs_err::tokio as fs;
use ship_types::{
    AgentKind, AgentSnapshot, AutonomyMode, CurrentTask, PersistedSession, ProjectName,
    SessionConfig, SessionId, SessionStartupState, TaskRecord,
};

use crate::{SessionStore, StoreError};

// r[testability.persistence-trait]
#[derive(Debug, Clone)]
pub struct JsonSessionStore {
    dir: PathBuf,
}

impl JsonSessionStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn session_path(&self, id: &SessionId) -> PathBuf {
        self.dir.join(format!("{}.json", id.0))
    }
}

#[derive(Debug, Clone, facet::Facet)]
struct LegacySessionConfig {
    project: ProjectName,
    base_branch: String,
    branch_name: String,
    captain_kind: AgentKind,
    mate_kind: AgentKind,
    autonomy_mode: AutonomyMode,
}

#[derive(Debug, Clone, facet::Facet)]
struct LegacyPersistedSession {
    id: SessionId,
    config: LegacySessionConfig,
    captain: AgentSnapshot,
    mate: AgentSnapshot,
    current_task: Option<CurrentTask>,
    task_history: Vec<TaskRecord>,
}

impl From<LegacyPersistedSession> for PersistedSession {
    fn from(value: LegacyPersistedSession) -> Self {
        Self {
            id: value.id,
            created_at: String::new(),
            config: SessionConfig {
                project: value.config.project,
                base_branch: value.config.base_branch,
                branch_name: value.config.branch_name,
                captain_kind: value.config.captain_kind,
                mate_kind: value.config.mate_kind,
                autonomy_mode: value.config.autonomy_mode,
                mcp_servers: Vec::new(),
            },
            captain: value.captain,
            mate: value.mate,
            startup_state: SessionStartupState::Ready,
            session_event_log: Vec::new(),
            current_task: value.current_task,
            task_history: value.task_history,
            title: None,
            archived_at: None,
            captain_acp_session_id: None,
            mate_acp_session_id: None,
        }
    }
}

fn decode_persisted_session(
    bytes: &[u8],
    display_name: &str,
) -> Result<PersistedSession, StoreError> {
    if let Ok(session) = facet_json::from_slice::<PersistedSession>(bytes) {
        return Ok(session);
    }

    if let Ok(session) = facet_json::from_slice::<LegacyPersistedSession>(bytes) {
        return Ok(session.into());
    }

    let current_error = facet_json::from_slice::<PersistedSession>(bytes)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_else(|| "unknown error".to_owned());
    Err(StoreError {
        message: format!("failed to deserialize {display_name}: {current_error}"),
    })
}

impl SessionStore for JsonSessionStore {
    async fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError> {
        fs::create_dir_all(&self.dir)
            .await
            .map_err(|error| StoreError {
                message: error.to_string(),
            })?;
        let bytes = facet_json::to_vec_pretty(session).map_err(|error| StoreError {
            message: format!("failed to serialize session {}: {error}", session.id.0),
        })?;
        fs::write(self.session_path(&session.id), bytes)
            .await
            .map_err(|error| StoreError {
                message: error.to_string(),
            })
    }

    async fn load_session(&self, id: &SessionId) -> Result<Option<PersistedSession>, StoreError> {
        let path = self.session_path(id);
        let bytes = match fs::read(path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(StoreError {
                    message: error.to_string(),
                });
            }
        };

        let session = decode_persisted_session(&bytes, &format!("session {}", id.0))?;
        Ok(Some(session))
    }

    // r[session.persistent.across-restart]
    async fn list_sessions(&self) -> Result<Vec<PersistedSession>, StoreError> {
        let mut out = Vec::new();
        let mut entries = match fs::read_dir(&self.dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(out),
            Err(error) => {
                return Err(StoreError {
                    message: error.to_string(),
                });
            }
        };

        while let Some(entry) = entries.next_entry().await.map_err(|error| StoreError {
            message: error.to_string(),
        })? {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let bytes = match fs::read(&path).await {
                Ok(b) => b,
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "skipping unreadable session file");
                    continue;
                }
            };

            let session = match decode_persisted_session(&bytes, &path.display().to_string()) {
                Ok(s) => s,
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error.message, "skipping unparseable session file");
                    continue;
                }
            };

            if session.archived_at.is_some() {
                continue;
            }

            out.push(session);
        }

        Ok(out)
    }

    async fn delete_session(&self, id: &SessionId) -> Result<(), StoreError> {
        let path = self.session_path(id);
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StoreError {
                message: error.to_string(),
            }),
        }
    }
}
