use std::sync::Arc;

use ship_agent::RoomReader;
use ship_db::ShipDb;
use ship_policy::{Block, RoomId};

/// A [`RoomReader`] backed by [`ShipDb`].
///
/// Shared (via `Arc`) between the runtime and any spawned agents so they
/// can read recent block history for context injection.
pub struct RuntimeRoomReader {
    db: Arc<ShipDb>,
}

impl RuntimeRoomReader {
    pub fn new(db: Arc<ShipDb>) -> Self {
        Self { db }
    }
}

impl RoomReader for RuntimeRoomReader {
    async fn recent_blocks(&self, room_id: &RoomId, limit: usize) -> Vec<Block> {
        let db = Arc::clone(&self.db);
        let room_id = room_id.clone();
        // ShipDb methods are synchronous (SQLite behind a Mutex), so we
        // use spawn_blocking to avoid holding the executor thread.
        let result = tokio::task::spawn_blocking(move || db.list_blocks(&room_id))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default();

        // Take the last `limit` blocks.
        if result.len() <= limit {
            result
        } else {
            result[result.len() - limit..].to_vec()
        }
    }
}
