use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for the server.
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
}

impl ServerConfig {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_owned(),
            port,
            max_connections: 100,
        }
    }

    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new("127.0.0.1", 8080)
    }
}

/// A session managed by the server.
pub struct Session {
    pub id: String,
    pub user: String,
    created_at: u64,
}

impl Session {
    pub fn new(id: &str, user: &str) -> Self {
        Self {
            id: id.to_owned(),
            user: user.to_owned(),
            created_at: 0,
        }
    }

    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.created_at)
    }
}

/// The main server that handles connections and sessions.
pub struct Server {
    config: ServerConfig,
    sessions: HashMap<String, Session>,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
        }
    }

    pub fn add_session(&mut self, session: Session) {
        self.sessions.insert(session.id.clone(), session);
    }

    pub fn remove_session(&mut self, id: &str) -> Option<Session> {
        self.sessions.remove(id)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_full(&self) -> bool {
        self.sessions.len() >= self.config.max_connections
    }

    /// Find sessions belonging to a specific user.
    pub fn find_user_sessions(&self, user: &str) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|s| s.user == user)
            .collect()
    }
}

/// Error types for server operations.
#[derive(Debug)]
pub enum ServerError {
    ConnectionRefused(String),
    SessionNotFound(String),
    MaxConnectionsReached,
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionRefused(reason) => write!(f, "connection refused: {reason}"),
            Self::SessionNotFound(id) => write!(f, "session not found: {id}"),
            Self::MaxConnectionsReached => write!(f, "max connections reached"),
        }
    }
}

const MAX_SESSION_AGE: u64 = 3600;
const DEFAULT_TIMEOUT: u64 = 30;

trait SessionValidator {
    fn is_valid(&self, now: u64) -> bool;
    fn refresh(&mut self);
}

impl SessionValidator for Session {
    fn is_valid(&self, now: u64) -> bool {
        self.age_seconds(now) < MAX_SESSION_AGE
    }

    fn refresh(&mut self) {
        // Reset the created_at timestamp
    }
}

/// Helper to create a test server with some sessions.
#[cfg(test)]
fn create_test_server() -> Server {
    let config = ServerConfig::new("localhost", 9090);
    let mut server = Server::new(config);
    server.add_session(Session::new("s1", "alice"));
    server.add_session(Session::new("s2", "bob"));
    server.add_session(Session::new("s3", "alice"));
    server
}
