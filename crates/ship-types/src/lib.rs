pub mod ids {
    // r[proto.id.session]
    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    pub struct SessionId(pub ulid::Ulid);

    // r[proto.id.task]
    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    pub struct TaskId(pub ulid::Ulid);

    // r[proto.id.project]
    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    pub struct ProjectName(pub String);
}

pub mod agent {
    // r[session.agent.kind]
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum AgentKind {
        Claude,
        Codex,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum Role {
        Captain,
        Mate,
    }

    // r[agent-state.plan-step]
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum PlanStepStatus {
        Planned,
        InProgress,
        Completed,
        Failed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct PlanStep {
        pub description: String,
        pub status: PlanStepStatus,
    }

    // r[approval.request.content]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct PermissionRequest {
        pub permission_id: String,
        pub tool_name: String,
        pub arguments: String,
        pub description: String,
    }

    // r[agent-state.derived]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum AgentState {
        Working {
            plan: Option<Vec<PlanStep>>,
            activity: Option<String>,
        },
        Idle,
        AwaitingPermission {
            request: PermissionRequest,
        },
        ContextExhausted,
        Error {
            message: String,
        },
    }

    // r[agent-state.snapshot]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct AgentSnapshot {
        pub role: Role,
        pub kind: AgentKind,
        pub state: AgentState,
        pub context_remaining_percent: Option<u8>,
    }
}

pub mod task {
    use crate::ids::TaskId;

    // r[task.status.enum]
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum TaskStatus {
        Assigned,
        Working,
        ReviewPending,
        SteerPending,
        Accepted,
        Cancelled,
    }

    // r[task.status.terminal]
    impl TaskStatus {
        pub fn is_terminal(&self) -> bool {
            matches!(self, Self::Accepted | Self::Cancelled)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct TaskRecord {
        pub id: TaskId,
        pub description: String,
        pub status: TaskStatus,
    }
}

pub mod events {
    use crate::agent::{PermissionRequest, PlanStep, Role};
    use crate::task::TaskStatus;
    use crate::{AgentState, TaskId};

    // r[event.content-block.types]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum ContentBlock {
        Text {
            text: String,
        },
        ToolCallStart {
            tool_name: String,
            arguments: String,
        },
        ToolCallDone {
            tool_name: String,
            output: String,
            success: bool,
        },
        PlanUpdate {
            steps: Vec<PlanStep>,
        },
        Error {
            message: String,
        },
    }

    // r[event.subscribe]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SessionEvent {
        AgentStateChanged {
            role: Role,
            state: AgentState,
        },
        Content {
            role: Role,
            block: ContentBlock,
        },
        PermissionRequested {
            role: Role,
            request: PermissionRequest,
        },
        TaskStatusChanged {
            task_id: TaskId,
            status: TaskStatus,
        },
        ContextUpdated {
            role: Role,
            remaining_percent: u8,
        },
    }

    // r[event.subscribe.roam-channel]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SubscribeMessage {
        Event(SessionEvent),
        ReplayComplete,
    }
}

pub mod protocol {
    use crate::agent::{AgentKind, AgentSnapshot};
    use crate::ids::{ProjectName, SessionId};
    use crate::task::{TaskRecord, TaskStatus};

    // r[autonomy.toggle]
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum AutonomyMode {
        HumanInTheLoop,
        Autonomous,
    }

    // r[session.create]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CreateSessionRequest {
        pub project: ProjectName,
        pub captain_kind: AgentKind,
        pub mate_kind: AgentKind,
        pub base_branch: String,
        pub task_description: String,
    }

    // r[proto.create-session]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CreateSessionResponse {
        pub session_id: SessionId,
        pub task_id: crate::ids::TaskId,
    }

    // r[proto.list-projects]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct ProjectInfo {
        pub name: ProjectName,
        pub path: String,
        pub valid: bool,
        pub invalid_reason: Option<String>,
    }

    // r[session.list]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct SessionSummary {
        pub id: SessionId,
        pub project: ProjectName,
        pub branch_name: String,
        pub captain: AgentSnapshot,
        pub mate: AgentSnapshot,
        pub current_task_description: Option<String>,
        pub task_status: Option<TaskStatus>,
        pub autonomy_mode: AutonomyMode,
    }

    // r[proto.get-session]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct SessionDetail {
        pub id: SessionId,
        pub project: ProjectName,
        pub branch_name: String,
        pub captain: AgentSnapshot,
        pub mate: AgentSnapshot,
        pub current_task: Option<TaskRecord>,
        pub task_history: Vec<TaskRecord>,
        pub autonomy_mode: AutonomyMode,
        pub pending_steer: Option<String>,
    }
}

pub mod persistence {
    use crate::agent::{AgentKind, AgentSnapshot, Role};
    use crate::events::ContentBlock;
    use crate::ids::{ProjectName, SessionId};
    use crate::protocol::AutonomyMode;
    use crate::task::TaskRecord;

    #[derive(Debug, Clone)]
    pub struct SessionConfig {
        pub project: ProjectName,
        pub base_branch: String,
        pub branch_name: String,
        pub captain_kind: AgentKind,
        pub mate_kind: AgentKind,
        pub autonomy_mode: AutonomyMode,
    }

    // r[mate.output.persisted]
    #[derive(Debug, Clone)]
    pub struct TaskContentRecord {
        pub role: Role,
        pub block: ContentBlock,
    }

    #[derive(Debug, Clone)]
    pub struct CurrentTask {
        pub record: TaskRecord,
        pub content_history: Vec<TaskContentRecord>,
    }

    // r[session.persistent]
    #[derive(Debug, Clone)]
    pub struct PersistedSession {
        pub id: SessionId,
        pub config: SessionConfig,
        pub captain: AgentSnapshot,
        pub mate: AgentSnapshot,
        pub current_task: Option<CurrentTask>,
        pub task_history: Vec<TaskRecord>,
    }
}

pub use agent::{
    AgentKind, AgentSnapshot, AgentState, PermissionRequest, PlanStep, PlanStepStatus, Role,
};
pub use events::{ContentBlock, SessionEvent, SubscribeMessage};
pub use ids::{ProjectName, SessionId, TaskId};
pub use persistence::{CurrentTask, PersistedSession, SessionConfig, TaskContentRecord};
pub use protocol::{
    AutonomyMode, CreateSessionRequest, CreateSessionResponse, ProjectInfo, SessionDetail,
    SessionSummary,
};
pub use task::{TaskRecord, TaskStatus};
