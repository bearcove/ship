mod acp_client;
mod acp_driver;
mod acp_launcher;
mod fakes;
mod git_worktree;
mod json_store;
mod mcp;
mod project_registry;
mod session_manager;
mod session_naming;

use core::fmt;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use futures_core::Stream;
use ship_types::{
    AgentKind, EffortValue, McpServerConfig, PersistedSession, Role, SessionEvent, SessionId,
};

pub use acp_driver::AcpAgentDriver;
pub use acp_launcher::{
    AgentLauncher, BinaryPathProbe, SystemBinaryPathProbe, discover_agents, resolve_agent_launcher,
};
pub use fakes::{
    FakeAgentDriver, FakePromptScript, FakeSessionStore, FakeWorktreeOps, SpawnRecord,
};
pub use git_worktree::GitWorktreeOps;
pub use json_store::JsonSessionStore;
pub use mcp::{McpConfigError, resolve_mcp_servers};
pub use project_registry::{ProjectRegistry, ProjectRegistryError};
pub use session_manager::{
    ActiveSession, PendingEdit, PendingPermission, SessionManager, SessionManagerError,
    SessionStateView, apply_block_patch, apply_event, archive_terminal_task,
    coalesce_replay_events, current_task_status, rebuild_materialized_from_event_log,
    set_agent_state, transition_task,
};
pub use session_naming::SessionGitNames;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionConfig {
    pub worktree_path: PathBuf,
    pub mcp_servers: Vec<McpServerConfig>,
    /// ACP session ID from a previous run, used for session resume
    pub resume_session_id: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpawnInfo {
    pub handle: AgentHandle,
    pub model_id: Option<String>,
    pub available_models: Vec<String>,
    pub effort_config_id: Option<String>,
    pub effort_value_id: Option<String>,
    pub available_effort_values: Vec<EffortValue>,
    /// The ACP session ID, for persisting and later resuming
    pub acp_session_id: String,
    /// Whether the ACP session was resumed from a previous run
    pub was_resumed: bool,
    /// Negotiated ACP protocol version
    pub protocol_version: u16,
    pub agent_name: Option<String>,
    pub agent_version: Option<String>,
    pub cap_load_session: bool,
    pub cap_resume_session: bool,
    pub cap_prompt_image: bool,
    pub cap_prompt_audio: bool,
    pub cap_prompt_embedded_context: bool,
    pub cap_mcp_http: bool,
    pub cap_mcp_sse: bool,
}

// r[testability.agent-trait]
#[allow(async_fn_in_trait)]
pub trait AgentDriver: Send + Sync {
    async fn spawn(
        &self,
        kind: AgentKind,
        role: Role,
        config: &AgentSessionConfig,
    ) -> Result<AgentSpawnInfo, AgentError>;

    async fn prompt(
        &self,
        handle: &AgentHandle,
        parts: &[ship_types::PromptContentPart],
    ) -> Result<PromptResponse, AgentError>;

    async fn cancel(&self, handle: &AgentHandle) -> Result<(), AgentError>;

    fn notifications(
        &self,
        handle: &AgentHandle,
    ) -> Pin<Box<dyn Stream<Item = SessionEvent> + Send + '_>>;

    async fn resolve_permission(
        &self,
        handle: &AgentHandle,
        permission_id: &str,
        option_id: &str,
    ) -> Result<(), AgentError>;

    async fn set_model(&self, handle: &AgentHandle, model_id: &str) -> Result<(), AgentError>;

    async fn set_effort(
        &self,
        handle: &AgentHandle,
        config_id: &str,
        value_id: &str,
    ) -> Result<(), AgentError>;

    async fn kill(&self, handle: &AgentHandle) -> Result<(), AgentError>;
}

// r[testability.git-trait]
#[allow(async_fn_in_trait)]
pub trait WorktreeOps: Send + Sync {
    async fn create_worktree(
        &self,
        branch_name: &str,
        worktree_dir: &str,
        base_branch: &str,
        repo_root: &Path,
    ) -> Result<PathBuf, WorktreeError>;

    async fn remove_worktree(&self, path: &Path, force: bool) -> Result<(), WorktreeError>;

    async fn has_uncommitted_changes(&self, path: &Path) -> Result<bool, WorktreeError>;

    async fn list_branches(&self, repo_root: &Path) -> Result<Vec<String>, WorktreeError>;

    async fn delete_branch(
        &self,
        branch_name: &str,
        force: bool,
        repo_root: &Path,
    ) -> Result<(), WorktreeError>;

    /// Rebase the worktree's current branch onto `onto_branch`.
    /// Runs inside the worktree directory so git uses that checkout.
    /// On conflict, aborts the rebase and returns an error.
    async fn rebase_onto(
        &self,
        worktree_path: &Path,
        onto_branch: &str,
    ) -> Result<(), WorktreeError>;

    /// Reset the already-checked-out worktree branch in place so it matches
    /// `base_branch`. This is server-owned state management for starting fresh
    /// task work on the stable session branch.
    async fn reset_to_base(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<(), WorktreeError>;

    /// Fast-forward merge `branch` into the repo root's current branch.
    async fn merge_ff_only(&self, repo_root: &Path, branch: &str) -> Result<(), WorktreeError>;

    // r[proto.archive-session.safety-check]
    /// Returns the list of commits on `branch_name` not yet in `base_branch`.
    /// An empty list means the branch has been fully merged.
    /// Returns an error only if git itself fails unexpectedly.
    async fn branch_unmerged_commits(
        &self,
        branch_name: &str,
        base_branch: &str,
        repo_root: &Path,
    ) -> Result<Vec<String>, WorktreeError>;
}

// r[testability.persistence-trait]
#[allow(async_fn_in_trait)]
pub trait SessionStore: Send + Sync {
    async fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError>;

    async fn load_session(&self, id: &SessionId) -> Result<Option<PersistedSession>, StoreError>;

    async fn list_sessions(&self) -> Result<Vec<PersistedSession>, StoreError>;

    async fn delete_session(&self, id: &SessionId) -> Result<(), StoreError>;
}
