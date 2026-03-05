use std::io;
use std::path::PathBuf;

use fs_err::tokio as fs;
use ship_types::{PersistedSession, SessionId};

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

        let session =
            facet_json::from_slice::<PersistedSession>(&bytes).map_err(|error| StoreError {
                message: format!("failed to deserialize session {}: {error}", id.0),
            })?;
        Ok(Some(session))
    }

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

            let bytes = fs::read(&path).await.map_err(|error| StoreError {
                message: error.to_string(),
            })?;
            let session =
                facet_json::from_slice::<PersistedSession>(&bytes).map_err(|error| StoreError {
                    message: format!("failed to deserialize {}: {error}", path.display()),
                })?;
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
