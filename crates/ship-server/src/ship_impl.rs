use std::process::Command;
use std::sync::Arc;

use roam::Tx;
use ship_core::{
    AcpAgentDriver, GitWorktreeOps, JsonSessionStore, ProjectRegistry, SessionManager,
    SessionManagerError, SessionStateView,
};
use ship_service::Ship;
use ship_types::{
    AgentKind, AgentSnapshot, AgentState, AutonomyMode, CreateSessionRequest,
    CreateSessionResponse, ProjectInfo, ProjectName, Role, SessionDetail, SessionId,
    SessionSummary, SubscribeMessage, TaskId,
};
use tokio::sync::Mutex;
use tokio::sync::broadcast;

// r[server.multi-repo]
#[derive(Clone)]
pub struct ShipImpl {
    registry: Arc<Mutex<ProjectRegistry>>,
    manager: Arc<Mutex<SessionManager<AcpAgentDriver, GitWorktreeOps, JsonSessionStore>>>,
    repo_root: Arc<std::path::PathBuf>,
}

impl ShipImpl {
    pub fn new(
        registry: ProjectRegistry,
        sessions_dir: std::path::PathBuf,
        repo_root: std::path::PathBuf,
    ) -> Self {
        let agent_driver = AcpAgentDriver::new();

        let manager = SessionManager::new(
            agent_driver,
            GitWorktreeOps,
            JsonSessionStore::new(sessions_dir),
        );

        Self {
            registry: Arc::new(Mutex::new(registry)),
            manager: Arc::new(Mutex::new(manager)),
            repo_root: Arc::new(repo_root),
        }
    }

    fn fallback_agent(role: Role, kind: AgentKind) -> AgentSnapshot {
        AgentSnapshot {
            role,
            kind,
            state: AgentState::Error {
                message: "session not found".to_owned(),
            },
            context_remaining_percent: None,
        }
    }

    fn to_session_detail(view: SessionStateView) -> SessionDetail {
        SessionDetail {
            id: view.id,
            project: view.config.project,
            branch_name: view.config.branch_name,
            captain: view.captain,
            mate: view.mate,
            current_task: view.current_task.map(|task| task.record),
            task_history: view.task_history,
            autonomy_mode: view.autonomy_mode,
            pending_steer: None,
        }
    }

    fn fallback_session_detail(id: SessionId) -> SessionDetail {
        SessionDetail {
            id,
            project: ProjectName("unknown".to_owned()),
            branch_name: String::new(),
            captain: Self::fallback_agent(Role::Captain, AgentKind::Claude),
            mate: Self::fallback_agent(Role::Mate, AgentKind::Codex),
            current_task: None,
            task_history: Vec::new(),
            autonomy_mode: AutonomyMode::HumanInTheLoop,
            pending_steer: None,
        }
    }

    fn log_manager_error(action: &str, error: &SessionManagerError) {
        tracing::warn!(%action, %error, "session manager call failed");
    }
}

impl Ship for ShipImpl {
    async fn list_projects(&self) -> Vec<ProjectInfo> {
        self.registry.lock().await.list()
    }

    async fn add_project(&self, path: String) -> ProjectInfo {
        let mut registry = self.registry.lock().await;
        match registry.add(&path).await {
            Ok(project) => project,
            Err(error) => ProjectInfo {
                name: ProjectName(
                    path.rsplit('/')
                        .find(|segment| !segment.is_empty())
                        .unwrap_or("project")
                        .to_owned(),
                ),
                path,
                valid: false,
                invalid_reason: Some(error.to_string()),
            },
        }
    }

    async fn list_branches(&self, project: ProjectName) -> Vec<String> {
        let project_path = {
            let registry = self.registry.lock().await;
            registry.get(&project.0).map(|project| project.path)
        };

        let Some(project_path) = project_path else {
            return Vec::new();
        };

        let output = Command::new("git")
            .args(["-C", project_path.as_str(), "branch", "-a"])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .map(|line| line.strip_prefix("* ").unwrap_or(line))
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    async fn list_sessions(&self) -> Vec<SessionSummary> {
        self.manager.lock().await.list_sessions()
    }

    async fn get_session(&self, id: SessionId) -> SessionDetail {
        let result = self.manager.lock().await.get_session(&id);
        match result {
            Ok(view) => Self::to_session_detail(view),
            Err(error) => {
                Self::log_manager_error("get_session", &error);
                Self::fallback_session_detail(id)
            }
        }
    }

    async fn create_session(&self, req: CreateSessionRequest) -> CreateSessionResponse {
        let result = self
            .manager
            .lock()
            .await
            .create_session(req, self.repo_root.as_ref())
            .await;

        match result {
            Ok((session_id, task_id)) => CreateSessionResponse {
                session_id,
                task_id,
            },
            Err(error) => {
                Self::log_manager_error("create_session", &error);
                CreateSessionResponse {
                    session_id: SessionId::new(),
                    task_id: TaskId::new(),
                }
            }
        }
    }

    async fn assign(&self, session: SessionId, description: String) -> TaskId {
        let result = self
            .manager
            .lock()
            .await
            .assign(&session, description)
            .await;
        match result {
            Ok(task_id) => task_id,
            Err(error) => {
                Self::log_manager_error("assign", &error);
                TaskId::new()
            }
        }
    }

    async fn steer(&self, session: SessionId, content: String) {
        if let Err(error) = self.manager.lock().await.steer(&session, content).await {
            Self::log_manager_error("steer", &error);
        }
    }

    async fn accept(&self, session: SessionId) {
        if let Err(error) = self.manager.lock().await.accept(&session).await {
            Self::log_manager_error("accept", &error);
        }
    }

    async fn cancel(&self, session: SessionId) {
        if let Err(error) = self.manager.lock().await.cancel(&session).await {
            Self::log_manager_error("cancel", &error);
        }
    }

    async fn resolve_permission(&self, session: SessionId, permission_id: String, approved: bool) {
        if let Err(error) = self
            .manager
            .lock()
            .await
            .resolve_permission(&session, &permission_id, approved)
            .await
        {
            Self::log_manager_error("resolve_permission", &error);
        }
    }

    async fn retry_agent(&self, _session: SessionId, _role: Role) {}

    async fn close_session(&self, id: SessionId) {
        if let Err(error) = self.manager.lock().await.close_session(&id).await {
            Self::log_manager_error("close_session", &error);
        }
    }

    async fn subscribe_events(&self, session: SessionId, output: Tx<SubscribeMessage>) {
        let mut receiver = {
            let manager = self.manager.lock().await;
            match manager.subscribe(&session) {
                Ok(receiver) => receiver,
                Err(error) => {
                    Self::log_manager_error("subscribe_events", &error);
                    let _ = output.close(Default::default()).await;
                    return;
                }
            }
        };

        loop {
            match receiver.try_recv() {
                Ok(event) => {
                    if output.send(SubscribeMessage::Event(event)).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                    tracing::warn!(%skipped, "subscribe replay lagged before ReplayComplete");
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    let _ = output.send(SubscribeMessage::ReplayComplete).await;
                    let _ = output.close(Default::default()).await;
                    return;
                }
            }
        }

        if output.send(SubscribeMessage::ReplayComplete).await.is_err() {
            return;
        }

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if output.send(SubscribeMessage::Event(event)).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(%skipped, "subscribe live stream lagged");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    let _ = output.close(Default::default()).await;
                    return;
                }
            }
        }
    }
}
