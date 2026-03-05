pub mod ids {
    // r[proto.id.session]
    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    #[repr(transparent)]
    #[facet(transparent)]
    pub struct SessionId(pub String);

    impl Default for SessionId {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SessionId {
        pub fn new() -> Self {
            Self(ulid::Ulid::new().to_string())
        }
    }

    // r[proto.id.task]
    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    #[repr(transparent)]
    #[facet(transparent)]
    pub struct TaskId(pub String);

    impl Default for TaskId {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TaskId {
        pub fn new() -> Self {
            Self(ulid::Ulid::new().to_string())
        }
    }

    // r[event.block-id]
    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    #[repr(transparent)]
    #[facet(transparent)]
    pub struct BlockId(pub String);

    impl Default for BlockId {
        fn default() -> Self {
            Self::new()
        }
    }

    impl BlockId {
        pub fn new() -> Self {
            Self(ulid::Ulid::new().to_string())
        }
    }

    // r[proto.id.project]
    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    #[repr(transparent)]
    #[facet(transparent)]
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
    use crate::TaskId;
    use crate::agent::{AgentState, PlanStep, Role};
    use crate::ids::BlockId;
    use crate::task::TaskStatus;

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum ToolCallStatus {
        Running,
        Success,
        Failure,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum PermissionResolution {
        Approved,
        Denied,
    }

    // r[event.content-block.types]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum ContentBlock {
        Text {
            text: String,
        },
        ToolCall {
            tool_name: String,
            arguments: String,
            status: ToolCallStatus,
            result: Option<String>,
        },
        PlanUpdate {
            steps: Vec<PlanStep>,
        },
        Error {
            message: String,
        },
        Permission {
            tool_name: String,
            description: String,
            arguments: String,
            resolution: Option<PermissionResolution>,
        },
    }

    // r[event.patch]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum BlockPatch {
        // r[event.patch.text-append]
        TextAppend {
            text: String,
        },
        // r[event.patch.tool-call-update]
        ToolCallUpdate {
            status: ToolCallStatus,
            result: Option<String>,
        },
        // r[event.patch.plan-replace]
        PlanReplace {
            steps: Vec<PlanStep>,
        },
        // r[event.patch.permission-resolve]
        PermissionResolve {
            resolution: PermissionResolution,
        },
    }

    // r[event.subscribe]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SessionEvent {
        // r[event.append]
        BlockAppend {
            block_id: BlockId,
            role: Role,
            block: ContentBlock,
        },
        // r[event.patch]
        BlockPatch {
            block_id: BlockId,
            role: Role,
            patch: BlockPatch,
        },
        // r[event.agent-state-changed]
        AgentStateChanged {
            role: Role,
            state: AgentState,
        },
        // r[event.task-status-changed]
        TaskStatusChanged {
            task_id: TaskId,
            status: TaskStatus,
        },
        // r[event.context-updated]
        ContextUpdated {
            role: Role,
            remaining_percent: u8,
        },
        // r[event.task-started]
        TaskStarted {
            task_id: TaskId,
            description: String,
        },
    }

    // r[event.envelope]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct SessionEventEnvelope {
        pub seq: u64,
        pub event: SessionEvent,
    }

    // r[event.subscribe.roam-channel]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SubscribeMessage {
        Event(SessionEventEnvelope),
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

    // r[proto.close-session]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CloseSessionRequest {
        pub id: SessionId,
        pub force: bool,
    }

    // r[proto.close-session]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum CloseSessionResponse {
        Closed,
        RequiresConfirmation,
        NotFound,
        Failed { message: String },
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
    use crate::events::{ContentBlock, SessionEventEnvelope};
    use crate::ids::{BlockId, ProjectName, SessionId};
    use crate::protocol::AutonomyMode;
    use crate::task::TaskRecord;

    #[derive(Debug, Clone, facet::Facet)]
    pub struct SessionConfig {
        pub project: ProjectName,
        pub base_branch: String,
        pub branch_name: String,
        pub captain_kind: AgentKind,
        pub mate_kind: AgentKind,
        pub autonomy_mode: AutonomyMode,
    }

    // r[mate.output.persisted]
    #[derive(Debug, Clone, facet::Facet)]
    pub struct TaskContentRecord {
        pub block_id: BlockId,
        pub role: Role,
        pub block: ContentBlock,
    }

    #[derive(Debug, Clone, facet::Facet)]
    pub struct CurrentTask {
        pub record: TaskRecord,
        pub content_history: Vec<TaskContentRecord>,
        // r[backend.persistence-contents]
        pub event_log: Vec<SessionEventEnvelope>,
    }

    // r[session.persistent]
    #[derive(Debug, Clone, facet::Facet)]
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
pub use events::{
    BlockPatch, ContentBlock, PermissionResolution, SessionEvent, SessionEventEnvelope,
    SubscribeMessage, ToolCallStatus,
};
pub use ids::{BlockId, ProjectName, SessionId, TaskId};
pub use persistence::{CurrentTask, PersistedSession, SessionConfig, TaskContentRecord};
pub use protocol::{
    AutonomyMode, CloseSessionRequest, CloseSessionResponse, CreateSessionRequest,
    CreateSessionResponse, ProjectInfo, SessionDetail, SessionSummary,
};
pub use task::{TaskRecord, TaskStatus};
