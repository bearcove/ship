mod schema;
mod store;

pub use store::{ShipDb, StoreError};

/// Maximum number of activity entries to keep (matching the existing in-memory limit).
pub const ACTIVITY_LOG_MAX_ENTRIES: u64 = 200;
