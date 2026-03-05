pub mod ids {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct SessionId(pub ulid::Ulid);

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct TaskId(pub ulid::Ulid);

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct ProjectName(pub String);
}

pub mod agent {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AgentKind {
        Claude,
        Codex,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Role {
        Captain,
        Mate,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PlanStepStatus {
        Planned,
        InProgress,
        Completed,
        Failed,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PlanStep {
        pub description: String,
        pub status: PlanStepStatus,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PermissionRequest {
        pub permission_id: String,
        pub tool_name: String,
        pub arguments: String,
        pub description: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AgentSnapshot {
        pub role: Role,
        pub kind: AgentKind,
        pub state: AgentState,
        pub context_remaining_percent: Option<u8>,
    }
}

pub mod task {
    use crate::ids::TaskId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TaskStatus {
        Assigned,
        Working,
        ReviewPending,
        SteerPending,
        Accepted,
        Cancelled,
    }

    impl TaskStatus {
        pub fn is_terminal(&self) -> bool {
            matches!(self, Self::Accepted | Self::Cancelled)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
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

    #[derive(Debug, Clone, PartialEq, Eq)]
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

    #[derive(Debug, Clone, PartialEq, Eq)]
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
}

pub mod protocol {
    use crate::agent::{AgentKind, AgentSnapshot};
    use crate::ids::{ProjectName, SessionId};
    use crate::task::TaskStatus;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AutonomyMode {
        HumanInTheLoop,
        Autonomous,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CreateSessionRequest {
        pub project: ProjectName,
        pub captain_kind: AgentKind,
        pub mate_kind: AgentKind,
        pub base_branch: String,
        pub task_description: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
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
pub use events::{ContentBlock, SessionEvent};
pub use ids::{ProjectName, SessionId, TaskId};
pub use persistence::{CurrentTask, PersistedSession, SessionConfig, TaskContentRecord};
pub use protocol::{AutonomyMode, CreateSessionRequest, SessionSummary};
pub use task::{TaskRecord, TaskStatus};
