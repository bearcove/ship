use roam::Tx;
use ship_service::Ship;
use ship_types::{
    AgentKind, AgentSnapshot, AgentState, AutonomyMode, ContentBlock, CreateSessionRequest,
    CreateSessionResponse, PermissionRequest, PlanStep, PlanStepStatus, ProjectInfo, ProjectName,
    Role, SessionDetail, SessionEvent, SessionId, SessionSummary, SubscribeMessage, TaskId,
    TaskRecord, TaskStatus,
};
use ulid::Ulid;

#[derive(Clone, Debug, Default)]
pub struct FakeShip;

impl FakeShip {
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

impl Ship for FakeShip {
    async fn list_projects(&self) -> Vec<ProjectInfo> {
        vec![
            ProjectInfo {
                name: ProjectName("ship-backend".to_owned()),
                path: "/Users/amos/bearcove/ship-backend".to_owned(),
                valid: true,
                invalid_reason: None,
            },
            ProjectInfo {
                name: ProjectName("roam".to_owned()),
                path: "/Users/amos/bearcove/roam".to_owned(),
                valid: true,
                invalid_reason: None,
            },
        ]
    }

    async fn add_project(&self, path: String) -> ProjectInfo {
        let name = path
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("project")
            .to_owned();
        ProjectInfo {
            name: ProjectName(name),
            path,
            valid: true,
            invalid_reason: None,
        }
    }

    async fn list_branches(&self, _project: ProjectName) -> Vec<String> {
        vec![
            "main".to_owned(),
            "backend".to_owned(),
            "feature/ship-roam-service".to_owned(),
            "origin/main".to_owned(),
        ]
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
        let permission = PermissionRequest {
            permission_id: "perm_fake_1".to_owned(),
            tool_name: "exec_command".to_owned(),
            arguments: "{\"cmd\":\"cargo check --workspace\"}".to_owned(),
            description: "Run workspace check".to_owned(),
        };

        let replay_events = vec![
            SessionEvent::TaskStatusChanged {
                task_id: task_id.clone(),
                status: TaskStatus::Working,
            },
            SessionEvent::AgentStateChanged {
                role: Role::Mate,
                state: AgentState::Working {
                    plan: Some(vec![PlanStep {
                        description: "Implement FakeShip".to_owned(),
                        status: PlanStepStatus::Completed,
                    }]),
                    activity: Some("Writing server scaffolding".to_owned()),
                },
            },
            SessionEvent::Content {
                role: Role::Mate,
                block: ContentBlock::Text {
                    text: "Implemented fake ship service and websocket wiring".to_owned(),
                },
            },
            SessionEvent::PermissionRequested {
                role: Role::Mate,
                request: permission,
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

        let live_event = SessionEvent::TaskStatusChanged {
            task_id,
            status: TaskStatus::ReviewPending,
        };
        let _ = output.send(SubscribeMessage::Event(live_event)).await;
        let _ = output.close(Default::default()).await;
    }
}
