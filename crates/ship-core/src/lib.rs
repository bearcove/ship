mod agent_presets;
mod fakes;
mod git_worktree;
mod hook_runner;
mod json_store;
mod project_config;
mod project_registry;
mod session_manager;
mod session_naming;

use core::fmt;
use std::error::Error;
use std::path::{Path, PathBuf};

use ship_types::{PersistedSession, SessionId};

// Re-export ACP types and modules from ship-acp
pub use ship_acp::{
    AcpAgentDriver, AgentDriver, AgentError, AgentHandle, AgentSessionConfig, AgentSpawnInfo,
    BinaryPathProbe, FakeAgentDriver, FakePromptScript, McpConfigError, PromptResponse,
    SpawnRecord, StopReason, SystemBinaryPathProbe,
    discover_agents, resolve_agent_launcher, resolve_mcp_servers,
};
pub use ship_acp::launcher::AgentLauncher;

pub use agent_presets::{AgentPresetConfigError, load_agent_presets};
pub use fakes::{FakeSessionStore, FakeWorktreeOps};
pub use git_worktree::GitWorktreeOps;
pub use hook_runner::{HookOutcome, HooksRunError, run_hooks};
pub use json_store::JsonSessionStore;
pub use project_config::{ProjectConfigError, load_project_hooks};
pub use project_registry::{ProjectRegistry, ProjectRegistryError};
pub use session_manager::{
    ActiveSession, PendingEdit, PendingPermission, SessionManager, SessionManagerError,
    SessionStateView, apply_block_patch, apply_event, archive_terminal_task,
    coalesce_replay_events, current_task_status, rebuild_materialized_from_event_log,
    set_agent_state, transition_task,
};
pub use session_naming::SessionGitNames;

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

impl From<eyre::Report> for WorktreeError {
    fn from(e: eyre::Report) -> Self {
        Self {
            message: format!("{e:#}"),
        }
    }
}

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

/// Outcome of a rebase operation that may encounter conflicts.
pub enum RebaseOutcome {
    Clean,
    Conflict { files: Vec<String> },
}

// r[testability.git-trait]
#[async_trait::async_trait]
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

    async fn current_branch(&self, worktree_path: &Path) -> Result<String, WorktreeError>;

    async fn is_rebase_in_progress(&self, worktree_path: &Path) -> Result<bool, WorktreeError>;

    async fn unmerged_paths(&self, worktree_path: &Path) -> Result<Vec<String>, WorktreeError>;

    async fn tracked_conflict_marker_paths(
        &self,
        worktree_path: &Path,
    ) -> Result<Vec<String>, WorktreeError>;
    async fn tracked_conflict_marker_locations(
        &self,
        worktree_path: &Path,
    ) -> Result<Vec<String>, WorktreeError>;

    async fn review_diff(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<String, WorktreeError>;

    /// Stage all changes and create a commit with the given message.
    async fn commit_all(&self, worktree_path: &Path, message: &str) -> Result<(), WorktreeError>;

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

    /// Like `rebase_onto` but returns `RebaseOutcome::Conflict` instead of
    /// aborting on conflicts. Only aborts and returns `Err` on unexpected
    /// git failures. On conflict, leaves the rebase in-progress.
    async fn rebase_onto_conflict_ok(
        &self,
        worktree_path: &Path,
        onto_branch: &str,
    ) -> Result<RebaseOutcome, WorktreeError>;

    /// Stage all changes (`git add -A`) and continue a paused rebase.
    /// Returns `RebaseOutcome::Conflict` if more conflicts remain after the
    /// continue step, or `RebaseOutcome::Clean` when the rebase finishes.
    async fn rebase_continue(&self, worktree_path: &Path) -> Result<RebaseOutcome, WorktreeError>;

    async fn rebase_abort(&self, worktree_path: &Path) -> Result<(), WorktreeError>;

    /// Reset the already-checked-out worktree branch in place so it matches
    /// `base_branch`. This is server-owned state management for starting fresh
    /// task work on the stable session branch.
    async fn reset_to_base(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<(), WorktreeError>;

    /// Fast-forward `into_branch` to match `branch` using `git fetch . <branch>:<into_branch>`.
    /// This updates the ref directly without requiring `into_branch` to be checked out,
    /// and enforces fast-forward semantics (non-fast-forward updates are rejected).
    async fn merge_ff_only(
        &self,
        repo_root: &Path,
        branch: &str,
        into_branch: &str,
    ) -> Result<(), WorktreeError>;

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
#[async_trait::async_trait]
pub trait SessionStore: Send + Sync {
    async fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError>;

    async fn load_session(&self, id: &SessionId) -> Result<Option<PersistedSession>, StoreError>;

    async fn list_sessions(&self) -> Result<Vec<PersistedSession>, StoreError>;

    async fn delete_session(&self, id: &SessionId) -> Result<(), StoreError>;
}
