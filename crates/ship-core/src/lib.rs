mod fakes;
mod session_manager;

use core::fmt;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use futures_core::Stream;
use ship_types::{AgentKind, PersistedSession, Role, SessionEvent, SessionId};

pub use fakes::{
    FakeAgentDriver, FakePromptScript, FakeSessionStore, FakeWorktreeOps, SpawnRecord,
};
pub use session_manager::{
    ActiveSession, PendingPermission, SessionManager, SessionManagerError, SessionStateView,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentHandle {
    id: SessionId,
}

impl AgentHandle {
    pub(crate) fn new(id: SessionId) -> Self {
        Self { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    Cancelled,
    ContextExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptResponse {
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentError {
    pub message: String,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for AgentError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeError {
    pub message: String,
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for WorktreeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError {
    pub message: String,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for StoreError {}

#[allow(async_fn_in_trait)]
pub trait AgentDriver: Send + Sync {
    async fn spawn(
        &self,
        kind: AgentKind,
        role: Role,
        worktree_path: &Path,
    ) -> Result<AgentHandle, AgentError>;

    async fn prompt(
        &self,
        handle: &AgentHandle,
        content: &str,
    ) -> Result<PromptResponse, AgentError>;

    async fn cancel(&self, handle: &AgentHandle) -> Result<(), AgentError>;

    fn notifications(
        &self,
        handle: &AgentHandle,
    ) -> Pin<Box<dyn Stream<Item = SessionEvent> + Send + '_>>;

    async fn kill(&self, handle: &AgentHandle) -> Result<(), AgentError>;
}

#[allow(async_fn_in_trait)]
pub trait WorktreeOps: Send + Sync {
    async fn create_worktree(
        &self,
        session_id: &SessionId,
        base_branch: &str,
        slug: &str,
        repo_root: &Path,
    ) -> Result<PathBuf, WorktreeError>;

    async fn remove_worktree(&self, path: &Path) -> Result<(), WorktreeError>;

    async fn has_uncommitted_changes(&self, path: &Path) -> Result<bool, WorktreeError>;

    async fn list_branches(&self, repo_root: &Path) -> Result<Vec<String>, WorktreeError>;

    async fn delete_branch(
        &self,
        branch_name: &str,
        force: bool,
        repo_root: &Path,
    ) -> Result<(), WorktreeError>;
}

#[allow(async_fn_in_trait)]
pub trait SessionStore: Send + Sync {
    async fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError>;

    async fn load_session(&self, id: &SessionId) -> Result<Option<PersistedSession>, StoreError>;

    async fn list_sessions(&self) -> Result<Vec<PersistedSession>, StoreError>;

    async fn delete_session(&self, id: &SessionId) -> Result<(), StoreError>;
}
