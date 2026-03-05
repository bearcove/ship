use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures_core::Stream;
use futures_util::stream;
use ship_types::{AgentKind, PersistedSession, Role, SessionEvent, SessionId};
use ulid::Ulid;

use crate::{
    AgentDriver, AgentError, AgentHandle, PromptResponse, SessionStore, StopReason, StoreError,
    WorktreeError, WorktreeOps,
};

#[derive(Debug, Clone)]
pub struct SpawnRecord {
    pub kind: AgentKind,
    pub role: Role,
    pub worktree_path: PathBuf,
    pub handle: AgentHandle,
}

#[derive(Debug, Clone)]
pub struct FakePromptScript {
    pub expected_handle: Option<AgentHandle>,
    pub response: Result<PromptResponse, AgentError>,
    pub events: Vec<SessionEvent>,
}

#[derive(Default)]
struct FakeAgentDriverInner {
    scripts: VecDeque<FakePromptScript>,
    notifications: HashMap<AgentHandle, VecDeque<SessionEvent>>,
    spawns: Vec<SpawnRecord>,
    prompts: Vec<(AgentHandle, String)>,
    cancelled: Vec<AgentHandle>,
    killed: Vec<AgentHandle>,
}

#[derive(Clone, Default)]
pub struct FakeAgentDriver {
    inner: Arc<Mutex<FakeAgentDriverInner>>,
}

impl FakeAgentDriver {
    pub fn push_script(&self, script: FakePromptScript) {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .scripts
            .push_back(script);
    }

    pub fn push_response(&self, stop_reason: StopReason) {
        self.push_script(FakePromptScript {
            expected_handle: None,
            response: Ok(PromptResponse { stop_reason }),
            events: Vec::new(),
        });
    }

    pub fn queue_notifications(&self, handle: &AgentHandle, events: Vec<SessionEvent>) {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .notifications
            .entry(handle.clone())
            .or_default()
            .extend(events);
    }

    pub fn spawn_records(&self) -> Vec<SpawnRecord> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .spawns
            .clone()
    }

    pub fn prompt_log(&self) -> Vec<(AgentHandle, String)> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .prompts
            .clone()
    }

    pub fn cancelled_handles(&self) -> Vec<AgentHandle> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .cancelled
            .clone()
    }

    pub fn killed_handles(&self) -> Vec<AgentHandle> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .killed
            .clone()
    }
}

impl AgentDriver for FakeAgentDriver {
    async fn spawn(
        &self,
        kind: AgentKind,
        role: Role,
        worktree_path: &Path,
    ) -> Result<AgentHandle, AgentError> {
        let handle = AgentHandle::new(SessionId(Ulid::new()));

        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .spawns
            .push(SpawnRecord {
                kind,
                role,
                worktree_path: worktree_path.to_path_buf(),
                handle: handle.clone(),
            });

        Ok(handle)
    }

    async fn prompt(
        &self,
        handle: &AgentHandle,
        content: &str,
    ) -> Result<PromptResponse, AgentError> {
        let mut inner = self.inner.lock().expect("fake agent driver mutex poisoned");
        inner.prompts.push((handle.clone(), content.to_owned()));

        let script = inner
            .scripts
            .pop_front()
            .unwrap_or_else(|| FakePromptScript {
                expected_handle: None,
                response: Ok(PromptResponse {
                    stop_reason: StopReason::EndTurn,
                }),
                events: Vec::new(),
            });

        if let Some(expected) = script.expected_handle
            && expected != *handle
        {
            return Err(AgentError {
                message: "prompt called with unexpected handle".to_owned(),
            });
        }

        if !script.events.is_empty() {
            inner
                .notifications
                .entry(handle.clone())
                .or_default()
                .extend(script.events);
        }

        script.response
    }

    async fn cancel(&self, handle: &AgentHandle) -> Result<(), AgentError> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .cancelled
            .push(handle.clone());
        Ok(())
    }

    fn notifications(
        &self,
        handle: &AgentHandle,
    ) -> Pin<Box<dyn Stream<Item = SessionEvent> + Send + '_>> {
        let events = self
            .inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .notifications
            .remove(handle)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();

        Box::pin(stream::iter(events))
    }

    async fn kill(&self, handle: &AgentHandle) -> Result<(), AgentError> {
        self.inner
            .lock()
            .expect("fake agent driver mutex poisoned")
            .killed
            .push(handle.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeWorktreeInner {
    next_idx: usize,
    created: HashMap<PathBuf, (SessionId, String, String, PathBuf)>,
    removed: Vec<PathBuf>,
    dirty_flags: HashMap<PathBuf, bool>,
    branches: Vec<String>,
    deleted_branches: Vec<(String, bool, PathBuf)>,
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
            .clone()
    }
}

impl WorktreeOps for FakeWorktreeOps {
    async fn create_worktree(
        &self,
        session_id: &SessionId,
        base_branch: &str,
        slug: &str,
        repo_root: &Path,
    ) -> Result<PathBuf, WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");

        inner.next_idx += 1;
        let path = repo_root.join(format!(".worktrees/fake-{}", inner.next_idx));
        inner.created.insert(
            path.clone(),
            (
                session_id.clone(),
                base_branch.to_owned(),
                slug.to_owned(),
                repo_root.to_path_buf(),
            ),
        );

        Ok(path)
    }

    async fn remove_worktree(&self, path: &Path) -> Result<(), WorktreeError> {
        let mut inner = self.inner.lock().expect("fake worktree ops mutex poisoned");

        inner.created.remove(path);
        inner.removed.push(path.to_path_buf());
        Ok(())
    }

    async fn has_uncommitted_changes(&self, path: &Path) -> Result<bool, WorktreeError> {
        let inner = self.inner.lock().expect("fake worktree ops mutex poisoned");

        Ok(*inner.dirty_flags.get(path).unwrap_or(&false))
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
        self.inner
            .lock()
            .expect("fake worktree ops mutex poisoned")
            .deleted_branches
            .push((branch_name.to_owned(), force, repo_root.to_path_buf()));
        Ok(())
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
