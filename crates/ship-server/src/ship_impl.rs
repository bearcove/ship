use std::process::Command;
use std::sync::{Arc, Mutex};

use roam::Tx;
use ship_core::ProjectRegistry;
use ship_service::Ship;
use ship_types::{
    AgentKind, AgentSnapshot, AgentState, AutonomyMode, BlockId, ContentBlock,
    CreateSessionRequest, CreateSessionResponse, PlanStep, PlanStepStatus, ProjectInfo,
    ProjectName, Role, SessionDetail, SessionEvent, SessionEventEnvelope, SessionId,
    SessionSummary, SubscribeMessage, TaskId, TaskRecord, TaskStatus,
};
use ulid::Ulid;

// r[server.multi-repo]
#[derive(Clone, Debug)]
pub struct ShipImpl {
    registry: Arc<Mutex<ProjectRegistry>>,
}

impl ShipImpl {
    pub fn new(registry: ProjectRegistry) -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry)),
        }
    }

    fn fake_session_id() -> SessionId {
        SessionId(Ulid::new())
    }

    fn fake_task_id() -> TaskId {
        TaskId(Ulid::new())
    }

    fn fake_agent(role: Role, kind: AgentKind) -> AgentSnapshot {
        AgentSnapshot {
            role,
            kind,
            state: AgentState::Working {
                plan: Some(vec![
                    PlanStep {
                        description: "Inspect repository state".to_owned(),
                        status: PlanStepStatus::Completed,
                    },
                    PlanStep {
                        description: "Generate Ship protocol bindings".to_owned(),
                        status: PlanStepStatus::InProgress,
                    },
                ]),
                activity: Some("Running codegen and verification".to_owned()),
            },
            context_remaining_percent: Some(72),
        }
    }
}

impl Ship for ShipImpl {
    async fn list_projects(&self) -> Vec<ProjectInfo> {
        self.registry
            .lock()
            .expect("project registry mutex poisoned")
            .list()
    }

    async fn add_project(&self, path: String) -> ProjectInfo {
        let mut registry = self
            .registry
            .lock()
            .expect("project registry mutex poisoned");
        match registry.add(&path) {
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
            let registry = self
                .registry
                .lock()
                .expect("project registry mutex poisoned");
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
        let session_id = Self::fake_session_id();
        vec![SessionSummary {
            id: session_id,
            project: ProjectName("ship-backend".to_owned()),
            branch_name: "ship/01hw1abc/backend-rpc".to_owned(),
            captain: Self::fake_agent(Role::Captain, AgentKind::Claude),
            mate: Self::fake_agent(Role::Mate, AgentKind::Codex),
            current_task_description: Some(
                "Implement fake Ship service and wire a roam websocket server".to_owned(),
            ),
            task_status: Some(TaskStatus::Working),
            autonomy_mode: AutonomyMode::HumanInTheLoop,
        }]
    }

    async fn get_session(&self, id: SessionId) -> SessionDetail {
        let current_task = TaskRecord {
            id: Self::fake_task_id(),
            description: "Implement fake Ship service and wire a roam websocket server".to_owned(),
            status: TaskStatus::ReviewPending,
        };

        let task_history = vec![TaskRecord {
            id: Self::fake_task_id(),
            description: "Generate TypeScript bindings with cargo xtask codegen".to_owned(),
            status: TaskStatus::Accepted,
        }];

        SessionDetail {
            id,
            project: ProjectName("ship-backend".to_owned()),
            branch_name: "ship/01hw1abc/backend-rpc".to_owned(),
            captain: Self::fake_agent(Role::Captain, AgentKind::Claude),
            mate: Self::fake_agent(Role::Mate, AgentKind::Codex),
            current_task: Some(current_task),
            task_history,
            autonomy_mode: AutonomyMode::HumanInTheLoop,
            pending_steer: Some("Tighten proxy error handling and add smoke test".to_owned()),
        }
    }

    async fn create_session(&self, _req: CreateSessionRequest) -> CreateSessionResponse {
        CreateSessionResponse {
            session_id: Self::fake_session_id(),
            task_id: Self::fake_task_id(),
        }
    }

    async fn assign(&self, _session: SessionId, _description: String) -> TaskId {
        Self::fake_task_id()
    }

    async fn steer(&self, _session: SessionId, _content: String) {}

    async fn accept(&self, _session: SessionId) {}

    async fn cancel(&self, _session: SessionId) {}

    async fn resolve_permission(
        &self,
        _session: SessionId,
        _permission_id: String,
        _approved: bool,
    ) {
    }

    async fn retry_agent(&self, _session: SessionId, _role: Role) {}

    async fn close_session(&self, _id: SessionId) {}

    async fn subscribe_events(&self, _session: SessionId, output: Tx<SubscribeMessage>) {
        let task_id = Self::fake_task_id();
        let text_block_id = BlockId(Ulid::new());
        let permission_block_id = BlockId(Ulid::new());

        let replay_events = vec![
            SessionEventEnvelope {
                seq: 0,
                event: SessionEvent::TaskStarted {
                    task_id: task_id.clone(),
                    description: "Implement fake Ship service and wire a roam websocket server"
                        .to_owned(),
                },
            },
            SessionEventEnvelope {
                seq: 1,
                event: SessionEvent::TaskStatusChanged {
                    task_id: task_id.clone(),
                    status: TaskStatus::Working,
                },
            },
            SessionEventEnvelope {
                seq: 2,
                event: SessionEvent::AgentStateChanged {
                    role: Role::Mate,
                    state: AgentState::Working {
                        plan: Some(vec![PlanStep {
                            description: "Implement ShipImpl".to_owned(),
                            status: PlanStepStatus::Completed,
                        }]),
                        activity: Some("Writing server scaffolding".to_owned()),
                    },
                },
            },
            SessionEventEnvelope {
                seq: 3,
                event: SessionEvent::BlockAppend {
                    block_id: text_block_id,
                    role: Role::Mate,
                    block: ContentBlock::Text {
                        text: "Implemented fake ship service and websocket wiring".to_owned(),
                    },
                },
            },
            SessionEventEnvelope {
                seq: 4,
                event: SessionEvent::BlockAppend {
                    block_id: permission_block_id,
                    role: Role::Mate,
                    block: ContentBlock::Permission {
                        tool_name: "exec_command".to_owned(),
                        description: "Run workspace check".to_owned(),
                        arguments: "{\"cmd\":\"cargo check --workspace\"}".to_owned(),
                        resolution: None,
                    },
                },
            },
        ];

        for event in replay_events {
            if output.send(SubscribeMessage::Event(event)).await.is_err() {
                return;
            }
        }

        if output.send(SubscribeMessage::ReplayComplete).await.is_err() {
            return;
        }

        let live_event = SessionEventEnvelope {
            seq: 5,
            event: SessionEvent::TaskStatusChanged {
                task_id,
                status: TaskStatus::ReviewPending,
            },
        };
        let _ = output.send(SubscribeMessage::Event(live_event)).await;
        let _ = output.close(Default::default()).await;
    }
}
