use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ship_types::{PersistedSession, SessionId};

use crate::{RebaseOutcome, SessionStore, StoreError, WorktreeError, WorktreeOps};

#[derive(Default)]
struct FakeWorktreeInner {
    next_idx: usize,
    created: HashMap<PathBuf, (String, String, String, PathBuf)>,
    removed: Vec<(PathBuf, bool)>,
    dirty_flags: HashMap<PathBuf, bool>,
    current_branches: HashMap<PathBuf, String>,
    rebase_in_progress: HashMap<PathBuf, bool>,
    unmerged_paths: HashMap<PathBuf, Vec<String>>,
    conflict_marker_paths: HashMap<PathBuf, Vec<String>>,
    conflict_marker_locations: HashMap<PathBuf, Vec<String>>,
    review_diffs: HashMap<PathBuf, String>,
    rebase_abort_requests: Vec<PathBuf>,
    branches: Vec<String>,
    deleted_branches: Vec<(String, bool, PathBuf)>,
    reset_requests: Vec<(PathBuf, String)>,
    remove_errors: HashMap<PathBuf, String>,
    delete_branch_errors: HashMap<String, String>,
    reset_errors: HashMap<PathBuf, String>,
    unmerged_commits: HashMap<String, Vec<String>>,
    commit_all_calls: Vec<(PathBuf, String)>,
    rebase_conflict_result: Option<Vec<String>>,
}

#[derive(Clone, Default)]
pub struct FakeWorktreeOps {
    inner: Arc<Mutex<FakeWorktreeInner>>,
}

impl FakeWorktreeOps {
    pub fn set_branches(&self, branches: Vec<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .branches = branches;
    }

    pub fn set_has_uncommitted_changes(&self, path: PathBuf, has_changes: bool) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .dirty_flags
            .insert(path, has_changes);
    }

    pub fn set_current_branch(&self, path: PathBuf, branch: impl Into<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .current_branches
            .insert(path, branch.into());
    }

    pub fn set_rebase_in_progress(&self, path: PathBuf, in_progress: bool) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .rebase_in_progress
            .insert(path, in_progress);
    }

    pub fn set_unmerged_paths(&self, path: PathBuf, files: Vec<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .unmerged_paths
            .insert(path, files);
    }

    pub fn set_conflict_marker_paths(&self, path: PathBuf, files: Vec<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .conflict_marker_paths
            .insert(path, files);
    }

    pub fn set_conflict_marker_locations(&self, path: PathBuf, locations: Vec<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .conflict_marker_locations
            .insert(path, locations);
    }

    pub fn set_review_diff(&self, path: PathBuf, diff: impl Into<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .review_diffs
            .insert(path, diff.into());
    }

    pub fn set_remove_error(&self, path: PathBuf, message: impl Into<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .remove_errors
            .insert(path, message.into());
    }

    pub fn set_delete_branch_error(
        &self,
        branch_name: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .delete_branch_errors
            .insert(branch_name.into(), message.into());
    }

    pub fn set_unmerged_commits(&self, branch_name: impl Into<String>, commits: Vec<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .unmerged_commits
            .insert(branch_name.into(), commits);
    }

    pub fn set_rebase_conflict(&self, files: Vec<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .rebase_conflict_result = Some(files);
    }

    pub fn created_paths(&self) -> Vec<PathBuf> {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .created
            .keys()
            .cloned()
            .collect()
    }

    pub fn removed_paths(&self) -> Vec<PathBuf> {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .removed
            .iter()
            .map(|(path, _force)| path.clone())
            .collect()
    }

    pub fn remove_requests(&self) -> Vec<(PathBuf, bool)> {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .removed
            .clone()
    }

    pub fn deleted_branches(&self) -> Vec<(String, bool, PathBuf)> {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .deleted_branches
            .clone()
    }

    pub fn set_reset_error(&self, path: PathBuf, message: impl Into<String>) {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .reset_errors
            .insert(path, message.into());
    }

    pub fn reset_requests(&self) -> Vec<(PathBuf, String)> {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .reset_requests
            .clone()
    }

    pub fn commit_all_calls(&self) -> Vec<(PathBuf, String)> {
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .commit_all_calls
            .clone()
    }
}

#[async_trait::async_trait]
impl WorktreeOps for FakeWorktreeOps {
    // r[worktree.path]
    async fn create_worktree(
        &self,
        branch_name: &str,
        worktree_dir: &str,
        base_branch: &str,
        repo_root: &Path,
    ) -> Result<PathBuf, WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");

        inner.next_idx += 1;
        let path = repo_root.join(format!(".ship/worktrees/fake-{}", inner.next_idx));
        inner.created.insert(
            path.clone(),
            (
                branch_name.to_owned(),
                worktree_dir.to_owned(),
                base_branch.to_owned(),
                repo_root.to_path_buf(),
            ),
        );
        if !inner.branches.iter().any(|b| b == branch_name) {
            inner.branches.push(branch_name.to_owned());
        }

        Ok(path)
    }

    async fn remove_worktree(&self, path: &Path, force: bool) -> Result<(), WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");

        if let Some(message) = inner.remove_errors.get(path) {
            return Err(WorktreeError {
                message: message.clone(),
            });
        }

        inner.created.remove(path);
        inner.removed.push((path.to_path_buf(), force));
        Ok(())
    }

    async fn has_uncommitted_changes(&self, path: &Path) -> Result<bool, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");

        Ok(*inner.dirty_flags.get(path).unwrap_or(&false))
    }

    async fn current_branch(&self, worktree_path: &Path) -> Result<String, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        Ok(inner
            .current_branches
            .get(worktree_path)
            .cloned()
            .unwrap_or_else(|| "HEAD".to_owned()))
    }

    async fn is_rebase_in_progress(&self, worktree_path: &Path) -> Result<bool, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        Ok(*inner
            .rebase_in_progress
            .get(worktree_path)
            .unwrap_or(&false))
    }

    async fn unmerged_paths(&self, worktree_path: &Path) -> Result<Vec<String>, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        Ok(inner
            .unmerged_paths
            .get(worktree_path)
            .cloned()
            .unwrap_or_default())
    }

    async fn tracked_conflict_marker_paths(
        &self,
        worktree_path: &Path,
    ) -> Result<Vec<String>, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        Ok(inner
            .conflict_marker_paths
            .get(worktree_path)
            .cloned()
            .unwrap_or_default())
    }

    async fn tracked_conflict_marker_locations(
        &self,
        worktree_path: &Path,
    ) -> Result<Vec<String>, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        if let Some(locations) = inner.conflict_marker_locations.get(worktree_path) {
            return Ok(locations.clone());
        }
        let fallback = inner
            .conflict_marker_paths
            .get(worktree_path)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|path| format!("{path}:1"))
            .collect();
        Ok(fallback)
    }

    async fn review_diff(
        &self,
        worktree_path: &Path,
        _base_branch: &str,
    ) -> Result<String, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        Ok(inner
            .review_diffs
            .get(worktree_path)
            .cloned()
            .unwrap_or_default())
    }

    async fn commit_all(&self, worktree_path: &Path, message: &str) -> Result<(), WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        inner
            .commit_all_calls
            .push((worktree_path.to_path_buf(), message.to_owned()));
        inner.dirty_flags.insert(worktree_path.to_path_buf(), false);
        Ok(())
    }

    async fn list_branches(&self, _repo_root: &Path) -> Result<Vec<String>, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");

        Ok(inner.branches.clone())
    }

    async fn delete_branch(
        &self,
        branch_name: &str,
        force: bool,
        repo_root: &Path,
    ) -> Result<(), WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        if let Some(message) = inner.delete_branch_errors.get(branch_name) {
            return Err(WorktreeError {
                message: message.clone(),
            });
        }
        inner
            .deleted_branches
            .push((branch_name.to_owned(), force, repo_root.to_path_buf()));
        inner.branches.retain(|branch| branch != branch_name);
        Ok(())
    }

    async fn rebase_onto(
        &self,
        _worktree_path: &Path,
        _onto_branch: &str,
    ) -> Result<(), WorktreeError> {
        Ok(())
    }

    async fn rebase_onto_conflict_ok(
        &self,
        _worktree_path: &Path,
        _onto_branch: &str,
    ) -> Result<RebaseOutcome, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        if let Some(files) = inner.rebase_conflict_result.clone() {
            return Ok(RebaseOutcome::Conflict { files });
        }
        Ok(RebaseOutcome::Clean)
    }

    async fn rebase_continue(&self, worktree_path: &Path) -> Result<RebaseOutcome, WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        if let Some(files) = inner.unmerged_paths.get(worktree_path).cloned()
            && !files.is_empty()
        {
            return Ok(RebaseOutcome::Conflict { files });
        }
        inner
            .rebase_in_progress
            .insert(worktree_path.to_path_buf(), false);
        Ok(RebaseOutcome::Clean)
    }

    async fn rebase_abort(&self, worktree_path: &Path) -> Result<(), WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        inner
            .rebase_abort_requests
            .push(worktree_path.to_path_buf());
        inner
            .rebase_in_progress
            .insert(worktree_path.to_path_buf(), false);
        Ok(())
    }

    async fn reset_to_base(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<(), WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        if let Some(message) = inner.reset_errors.get(worktree_path) {
            return Err(WorktreeError {
                message: message.clone(),
            });
        }
        inner
            .reset_requests
            .push((worktree_path.to_path_buf(), base_branch.to_owned()));
        inner.dirty_flags.insert(worktree_path.to_path_buf(), false);
        Ok(())
    }

    async fn merge_ff_only(
        &self,
        _repo_root: &Path,
        _branch: &str,
        _into_branch: &str,
    ) -> Result<(), WorktreeError> {
        Ok(())
    }

    async fn branch_unmerged_commits(
        &self,
        branch_name: &str,
        _base_branch: &str,
        _repo_root: &Path,
    ) -> Result<Vec<String>, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");
        Ok(inner
            .unmerged_commits
            .get(branch_name)
            .cloned()
            .unwrap_or_default())
    }
}

#[derive(Clone, Default)]
pub struct FakeSessionStore {
    sessions: Arc<Mutex<HashMap<SessionId, PersistedSession>>>,
}

impl FakeSessionStore {
    pub fn snapshot(&self) -> HashMap<SessionId, PersistedSession> {
        self.sessions
            .lock()
            .expect("fake session store mutex poisoned")
            .clone()
    }
}

#[async_trait::async_trait]
impl SessionStore for FakeSessionStore {
    async fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError> {
        self.sessions
            .lock()
            .expect("fake session store mutex poisoned")
            .insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn load_session(&self, id: &SessionId) -> Result<Option<PersistedSession>, StoreError> {
        Ok(self
            .sessions
            .lock()
            .expect("fake session store mutex poisoned")
            .get(id)
            .cloned())
    }

    async fn list_sessions(&self) -> Result<Vec<PersistedSession>, StoreError> {
        Ok(self
            .sessions
            .lock()
            .expect("fake session store mutex poisoned")
            .values()
            .filter(|s| s.archived_at.is_none())
            .cloned()
            .collect())
    }

    async fn delete_session(&self, id: &SessionId) -> Result<(), StoreError> {
        self.sessions
            .lock()
            .expect("fake session store mutex poisoned")
            .remove(id);
        Ok(())
    }
}
