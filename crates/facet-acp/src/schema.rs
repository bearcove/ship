mod agent;
mod client;
mod content;
mod error;
mod ext;
mod plan;
mod tool_call;

pub use agent::*;
pub use client::*;
pub use content::*;
pub use error::*;
pub use ext::*;
pub use plan::*;
pub use tool_call::*;

use std::sync::Arc;

use facet::Facet;

/// A unique identifier for a conversation session between a client and agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct SessionId(pub Arc<str>);

impl SessionId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self(Arc::from(s.as_str()))
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl From<Arc<str>> for SessionId {
    fn from(s: Arc<str>) -> Self {
        Self(s)
    }
}

/// Protocol version identifier. Only bumped for breaking changes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Facet)]
#[facet(transparent)]
pub struct ProtocolVersion(pub u16);

impl ProtocolVersion {
    pub const V0: Self = Self(0);
    pub const V1: Self = Self(1);
    pub const LATEST: Self = Self::V1;
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u16> for ProtocolVersion {
    fn from(v: u16) -> Self {
        Self(v)
    }
}
