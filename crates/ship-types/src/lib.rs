// ship-types: shared types for the Ship system
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
    use crate::structured::{JsonValue, PermissionOption, ToolCallKind, ToolTarget};

    // r[session.agent.kind]
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum AgentKind {
        Claude,
        Codex,
        OpenCode,
    }

    impl AgentKind {
        pub fn default_provider_id(self) -> AgentProviderId {
            match self {
                Self::Claude => AgentProviderId("anthropic".to_owned()),
                Self::Codex => AgentProviderId("openai".to_owned()),
                Self::OpenCode => AgentProviderId("openrouter".to_owned()),
            }
        }
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
        Pending,
        InProgress,
        Completed,
        Failed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct PlanStep {
        #[facet(default)]
        pub title: String,
        pub description: String,
        pub status: PlanStepStatus,
        #[facet(default)]
        pub started_at: Option<String>,
    }

    /// Input for creating a plan step (used by captain_assign and set_plan).
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct PlanStepInput {
        pub title: String,
        pub description: String,
    }

    // r[captain.tool.assign.files]
    /// A file reference supplied by the captain in captain_assign.
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct AssignFileRef {
        pub path: String,
        pub start_line: Option<u64>,
        pub end_line: Option<u64>,
    }

    // r[captain.tool.assign.dirty-session-strategy]
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum DirtySessionStrategy {
        ContinueInPlace,
        SaveAndStartClean,
    }

    // r[captain.tool.assign.files]
    // r[captain.tool.assign.plan]
    // r[captain.tool.assign.dirty-session-strategy]
    /// Extra optional parameters for captain_assign, bundled to stay within
    /// roam's 4-tuple serialization limit.
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CaptainAssignExtras {
        pub files: Vec<AssignFileRef>,
        pub plan: Vec<PlanStepInput>,
        pub dirty_session_strategy: Option<DirtySessionStrategy>,
    }

    // r[approval.request.content]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct PermissionRequest {
        pub permission_id: String,
        pub tool_call_id: Option<String>,
        pub tool_name: String,
        pub arguments: String,
        pub description: String,
        pub kind: Option<ToolCallKind>,
        pub target: Option<ToolTarget>,
        pub raw_input: Option<JsonValue>,
        pub options: Option<Vec<PermissionOption>>,
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
            request: Box<PermissionRequest>,
        },
        ContextExhausted,
        Error {
            message: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct EffortValue {
        pub id: String,
        pub name: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    #[repr(transparent)]
    #[facet(transparent)]
    pub struct AgentPresetId(pub String);

    #[derive(Debug, Clone, PartialEq, Eq, Hash, facet::Facet)]
    #[repr(transparent)]
    #[facet(transparent)]
    pub struct AgentProviderId(pub String);

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct AgentPreset {
        pub id: AgentPresetId,
        pub label: String,
        pub kind: AgentKind,
        pub provider: AgentProviderId,
        pub model_id: String,
        #[facet(default)]
        pub logo: Option<String>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, facet::Facet)]
    pub struct AgentPresetsConfig {
        #[facet(default)]
        pub presets: Vec<AgentPreset>,
    }

    // r[agent-state.snapshot]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct AgentSnapshot {
        pub role: Role,
        pub kind: AgentKind,
        pub state: AgentState,
        pub context_remaining_percent: Option<u8>,
        #[facet(default)]
        pub preset_id: Option<AgentPresetId>,
        #[facet(default)]
        pub provider: Option<AgentProviderId>,
        pub model_id: Option<String>,
        pub available_models: Vec<String>,
        pub effort_config_id: Option<String>,
        pub effort_value_id: Option<String>,
        pub available_effort_values: Vec<EffortValue>,
    }

    // r[acp.debug-info]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct AgentAcpInfo {
        pub acp_session_id: String,
        pub was_resumed: bool,
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
        pub last_event_at: Option<String>,
    }
}

pub mod structured {
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct JsonEntry {
        pub key: String,
        pub value: JsonValue,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum JsonValue {
        Null,
        Bool { value: bool },
        Number { value: String },
        String { value: String },
        Array { items: Vec<JsonValue> },
        Object { entries: Vec<JsonEntry> },
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum ToolCallKind {
        Read,
        Edit,
        Delete,
        Move,
        Search,
        Execute,
        Think,
        Fetch,
        SwitchMode,
        Other,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum ToolTarget {
        None,
        File {
            path: String,
            display_path: Option<String>,
            line: Option<u32>,
        },
        Move {
            source_path: String,
            source_display_path: Option<String>,
            destination_path: String,
            destination_display_path: Option<String>,
        },
        Search {
            query: Option<String>,
            path: Option<String>,
            display_path: Option<String>,
            glob: Option<String>,
        },
        Command {
            command: String,
            cwd: Option<String>,
            display_cwd: Option<String>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct TerminalExit {
        pub exit_code: Option<u32>,
        pub signal: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct TerminalSnapshot {
        pub output: String,
        pub truncated: bool,
        pub exit: Option<TerminalExit>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct ToolCallError {
        pub message: String,
        pub details: Option<JsonValue>,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum PermissionOptionKind {
        AllowOnce,
        AllowAlways,
        RejectOnce,
        RejectAlways,
        Other,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct PermissionOption {
        pub option_id: String,
        pub label: String,
        pub kind: PermissionOptionKind,
    }
}

pub mod task {
    use crate::agent::PlanStep;
    use crate::ids::TaskId;

    // r[task.status.enum]
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum TaskStatus {
        Assigned,
        Working,
        ReviewPending,
        SteerPending,
        RebaseConflict,
        Accepted,
        Cancelled,
        WaitingForHuman,
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
        pub title: String,
        pub description: String,
        pub status: TaskStatus,
        #[facet(default)]
        pub steps: Vec<PlanStep>,
        #[facet(default)]
        pub assigned_at: Option<String>,
        #[facet(default)]
        pub completed_at: Option<String>,
    }
}

pub mod session {
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum SessionStartupStage {
        ResolvingMcp,
        CreatingWorktree,
        StartingCaptain,
        StartingMate,
        GreetingCaptain,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SessionStartupState {
        Pending,
        Running {
            stage: SessionStartupStage,
            message: String,
        },
        Ready,
        Failed {
            stage: SessionStartupStage,
            message: String,
        },
    }
}

pub mod events {
    use crate::TaskId;
    use crate::agent::{
        AgentAcpInfo, AgentKind, AgentPresetId, AgentProviderId, AgentState, EffortValue, PlanStep,
        Role,
    };
    use crate::ids::BlockId;
    use crate::protocol::{ProjectInfo, SessionSummary};
    use crate::session::SessionStartupState;
    use crate::structured::{JsonValue, TerminalSnapshot, ToolCallError, ToolCallKind, ToolTarget};
    use crate::task::TaskStatus;

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum TextSource {
        Human,
        AgentMessage,
        AgentThought,
        Steer,
    }

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

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct ToolCallLocation {
        pub path: String,
        pub display_path: Option<String>,
        pub line: Option<u32>,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum ToolCallContent {
        Text {
            text: String,
        },
        Diff {
            path: String,
            display_path: Option<String>,
            unified_diff: String,
        },
        Terminal {
            terminal_id: String,
            snapshot: Option<TerminalSnapshot>,
        },
        Raw {
            data: JsonValue,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CommitSummary {
        pub hash: String,
        pub subject: String,
        #[facet(default)]
        pub diff: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct TaskRecapStats {
        pub files_changed: u32,
        pub insertions: u32,
        pub deletions: u32,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum WorkflowMilestoneKind {
        PlanSet,
        StepCommitted,
        ReviewSubmitted,
        RebaseConflict,
        TaskAccepted,
    }

    // r[event.content-block.types]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum ContentBlock {
        Text {
            text: String,
            source: TextSource,
        },
        ToolCall {
            tool_call_id: Option<String>,
            tool_name: String,
            arguments: String,
            kind: Option<ToolCallKind>,
            target: Option<ToolTarget>,
            raw_input: Option<JsonValue>,
            raw_output: Option<JsonValue>,
            locations: Vec<ToolCallLocation>,
            status: ToolCallStatus,
            content: Vec<ToolCallContent>,
            error: Option<ToolCallError>,
        },
        PlanUpdate {
            steps: Vec<PlanStep>,
        },
        Error {
            message: String,
        },
        Permission {
            permission_id: Option<String>,
            tool_call_id: Option<String>,
            tool_name: String,
            description: String,
            arguments: String,
            kind: Option<ToolCallKind>,
            target: Option<ToolTarget>,
            raw_input: Option<JsonValue>,
            options: Option<Vec<crate::structured::PermissionOption>>,
            resolution: Option<PermissionResolution>,
        },
        Image {
            mime_type: String,
            data: Vec<u8>,
        },
        WorkflowMilestone {
            kind: WorkflowMilestoneKind,
            title: String,
            summary: String,
            #[facet(default)]
            items: Vec<String>,
            #[facet(default)]
            commits: Vec<CommitSummary>,
            #[facet(default)]
            stats: Option<TaskRecapStats>,
        },
        // r[task.recap]
        TaskRecap {
            commits: Vec<CommitSummary>,
            stats: Option<TaskRecapStats>,
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
            tool_name: Option<String>,
            kind: Option<ToolCallKind>,
            target: Box<Option<ToolTarget>>,
            raw_input: Option<JsonValue>,
            raw_output: Option<JsonValue>,
            status: ToolCallStatus,
            locations: Option<Vec<ToolCallLocation>>,
            content: Option<Vec<ToolCallContent>>,
            error: Option<ToolCallError>,
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
        SessionStartupChanged {
            state: SessionStartupState,
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
            title: String,
            description: String,
            #[facet(default)]
            steps: Vec<PlanStep>,
        },
        AgentModelChanged {
            role: Role,
            model_id: Option<String>,
            available_models: Vec<String>,
        },
        AgentPresetChanged {
            role: Role,
            preset_id: Option<AgentPresetId>,
            kind: AgentKind,
            provider: Option<AgentProviderId>,
        },
        AgentEffortChanged {
            role: Role,
            effort_config_id: Option<String>,
            effort_value_id: Option<String>,
            available_effort_values: Vec<EffortValue>,
        },
        /// A built-in tool was blocked; the session manager should inject this
        /// message into the agent's next prompt turn so it knows to use MCP
        /// tools instead. Role indicates which agent triggered it.
        MateGuidanceQueued {
            role: Role,
            message: String,
        },
        // r[event.human-review-requested]
        /// Captain called captain_notify_human; waiting for human to approve/respond.
        HumanReviewRequested {
            message: String,
            /// Post-rebase diff showing what would merge right now
            diff: String,
            /// Absolute path to the session worktree for manual inspection
            worktree_path: String,
        },
        // r[event.human-review-cleared]
        /// Human responded; captain is unblocked.
        HumanReviewCleared,
        // r[event.session-title-changed]
        /// Auto-generated session title set after the first user message.
        SessionTitleChanged {
            title: String,
        },
        // r[acp.debug-info]
        AgentAcpInfoChanged {
            role: Role,
            info: AgentAcpInfo,
        },
        /// Project checks have started running.
        ChecksStarted {
            /// e.g. "post-commit", "pre-merge", "worktree-setup"
            context: String,
            /// Names of the hooks being run.
            hooks: Vec<String>,
        },
        /// Project checks have finished.
        ChecksFinished {
            context: String,
            all_passed: bool,
            results: Vec<HookCheckResult>,
        },
    }

    /// Outcome of a single hook within a checks run.
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct HookCheckResult {
        pub name: String,
        pub passed: bool,
        /// Combined stdout/stderr — populated on failure, empty on success.
        pub output: String,
    }

    // r[event.envelope]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct SessionEventEnvelope {
        pub seq: u64,
        pub timestamp: String,
        pub event: SessionEvent,
    }

    // r[event.subscribe.roam-channel]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SubscribeMessage {
        Event(Box<SessionEventEnvelope>),
        ReplayComplete,
    }

    // r[proto.activity-entry]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct ActivityEntry {
        pub id: u64,
        pub timestamp: String,
        pub session_id: crate::ids::SessionId,
        pub session_slug: String,
        pub session_title: Option<String>,
        pub kind: ActivityKind,
    }

    // r[proto.activity-kind]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum ActivityKind {
        CaptainMessage { message: String },
        AdmiralMessage { message: String },
        SessionCreated,
        SessionArchived,
    }

    // r[proto.subscribe-global-events]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum GlobalEvent {
        SessionListChanged { sessions: Vec<SessionSummary> },
        ProjectListChanged { projects: Vec<ProjectInfo> },
        Activity { entry: ActivityEntry },
    }
}

pub mod protocol {
    use crate::agent::{AgentAcpInfo, AgentKind, AgentSnapshot};
    use crate::ids::{ProjectName, SessionId};
    use crate::session::SessionStartupState;
    use crate::task::{TaskRecord, TaskStatus};

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct McpHeader {
        pub name: String,
        pub value: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct McpEnvVar {
        pub name: String,
        pub value: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct McpHttpServerConfig {
        pub name: String,
        pub url: String,
        pub headers: Vec<McpHeader>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct McpSseServerConfig {
        pub name: String,
        pub url: String,
        pub headers: Vec<McpHeader>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct McpStdioServerConfig {
        pub name: String,
        pub command: String,
        pub args: Vec<String>,
        pub env: Vec<McpEnvVar>,
    }

    // r[acp.mcp.config]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum McpServerConfig {
        Http(McpHttpServerConfig),
        Sse(McpSseServerConfig),
        Stdio(McpStdioServerConfig),
    }

    // r[autonomy.toggle]
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum AutonomyMode {
        HumanInTheLoop,
        Autonomous,
    }

    #[derive(Debug, Clone, facet::Facet)]
    pub struct NewSessionDefaults {
        pub project: ProjectName,
        #[facet(default)]
        pub captain_preset_id: Option<crate::agent::AgentPresetId>,
        #[facet(default)]
        pub mate_preset_id: Option<crate::agent::AgentPresetId>,
    }

    // r[session.create]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CreateSessionRequest {
        pub project: ProjectName,
        pub captain_kind: AgentKind,
        pub mate_kind: AgentKind,
        #[facet(default)]
        pub captain_preset_id: Option<crate::agent::AgentPresetId>,
        #[facet(default)]
        pub mate_preset_id: Option<crate::agent::AgentPresetId>,
        pub base_branch: String,
        pub mcp_servers: Option<Vec<McpServerConfig>>,
    }

    // r[proto.create-session]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum CreateSessionResponse {
        Created { session_id: SessionId, slug: String },
        Failed { message: String },
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct McpToolCallResponse {
        pub text: String,
        pub is_error: bool,
        pub diffs: Vec<McpDiffContent>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct McpDiffContent {
        pub path: String,
        /// Compact unified diff (±3 context lines) showing what changed.
        pub unified_diff: String,
        /// For edit_prepare responses: the edit_id to pass to edit_confirm.
        pub edit_id: Option<String>,
    }

    // r[proto.close-session]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CloseSessionRequest {
        pub id: SessionId,
        pub force: bool,
    }

    // r[proto.archive-session]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct ArchiveSessionRequest {
        pub id: SessionId,
        pub force: bool,
    }

    // r[proto.archive-session]
    // r[proto.archive-session.safety-check]
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum ArchiveSessionResponse {
        Archived,
        RequiresConfirmation { unmerged_commits: Vec<String> },
        NotFound,
        Failed { message: String },
    }

    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SetAgentModelResponse {
        Ok,
        SessionNotFound,
        AgentNotSpawned,
        Failed { message: String },
    }

    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SetAgentPresetResponse {
        Ok,
        SessionNotFound,
        AgentNotSpawned,
        PresetNotFound,
        Failed { message: String },
    }

    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub enum SetAgentEffortResponse {
        Ok,
        SessionNotFound,
        AgentNotSpawned,
        Failed { message: String },
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

    // r[server.agent-discovery]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct AgentDiscovery {
        pub claude: bool,
        pub codex: bool,
        pub opencode: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct ServerInfo {
        /// All HTTP URLs the server is listening on, non-loopback first.
        pub http_urls: Vec<String>,
    }

    // r[session.list]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct SessionSummary {
        pub id: SessionId,
        pub slug: String,
        pub project: ProjectName,
        pub branch_name: String,
        pub title: Option<String>,
        pub captain: AgentSnapshot,
        pub mate: AgentSnapshot,
        pub startup_state: SessionStartupState,
        pub current_task_title: Option<String>,
        pub current_task_description: Option<String>,
        pub task_status: Option<TaskStatus>,
        pub diff_stats: Option<WorktreeDiffStats>,
        pub tasks_done: u32,
        pub tasks_total: u32,
        pub autonomy_mode: AutonomyMode,
        pub created_at: String,
        #[facet(default)]
        pub is_admiral: bool,
    }

    // r[event.human-review-requested]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct HumanReviewRequest {
        pub message: String,
        /// Post-rebase diff showing what would merge right now
        pub diff: String,
        /// Absolute path to the session worktree for manual inspection
        pub worktree_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CaptainGitStatus {
        pub branch_name: String,
        pub base_branch: String,
        pub is_dirty: bool,
        pub rebase_in_progress: bool,
        pub unmerged_paths: Vec<String>,
        pub conflict_marker_paths: Vec<String>,
        pub conflict_marker_locations: Vec<String>,
        pub review_safe: bool,
        pub merge_safe: bool,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum CaptainReviewDiffState {
        Ready,
        RebaseConflict,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CaptainReviewDiff {
        pub state: CaptainReviewDiffState,
        pub status: CaptainGitStatus,
        pub diff: String,
        pub conflicted_files: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CaptainRebaseStatus {
        pub status: CaptainGitStatus,
        pub can_continue: bool,
        pub can_abort: bool,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum CaptainRebaseAction {
        Continue,
        Abort,
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
    pub enum CaptainRebaseActionOutcome {
        Blocked,
        Conflict,
        Completed,
        Aborted,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct CaptainRebaseActionResult {
        pub action: CaptainRebaseAction,
        pub outcome: CaptainRebaseActionOutcome,
        pub status: CaptainRebaseStatus,
        pub conflicted_files: Vec<String>,
    }

    // r[proto.get-session]
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct SessionDetail {
        pub id: SessionId,
        pub slug: String,
        pub project: ProjectName,
        pub branch_name: String,
        pub title: Option<String>,
        pub captain: AgentSnapshot,
        pub mate: AgentSnapshot,
        pub startup_state: SessionStartupState,
        pub current_task: Option<TaskRecord>,
        pub task_history: Vec<TaskRecord>,
        pub autonomy_mode: AutonomyMode,
        pub pending_steer: Option<String>,
        pub pending_human_review: Option<HumanReviewRequest>,
        pub created_at: String,
        pub user_avatar_url: Option<String>,
        #[facet(default)]
        pub captain_acp_info: Option<AgentAcpInfo>,
        #[facet(default)]
        pub mate_acp_info: Option<AgentAcpInfo>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct WorktreeDiffStats {
        pub branch_name: String,
        pub lines_added: u64,
        pub lines_removed: u64,
        pub files_changed: u64,
        pub uncommitted_lines_added: u64,
        pub uncommitted_lines_removed: u64,
    }
}

pub mod prompt {
    // r[ui.composer.image-attach]
    #[repr(u8)]
    #[derive(Debug, Clone, facet::Facet)]
    pub enum PromptContentPart {
        Text { text: String },
        Image { mime_type: String, data: Vec<u8> },
    }
}

pub mod transcription {
    /// A transcribed text segment from the whisper model.
    #[derive(Debug, Clone, PartialEq, facet::Facet)]
    pub struct TranscribeSegment {
        /// Start time in milliseconds
        pub start_ms: u64,
        /// End time in milliseconds
        pub end_ms: u64,
        /// The transcribed text
        pub text: String,
    }

    /// Messages sent over the transcription channel — either a segment or an error.
    #[repr(u8)]
    #[derive(Debug, Clone, PartialEq, facet::Facet)]
    pub enum TranscribeMessage {
        Segment(TranscribeSegment),
        Error { message: String },
    }
}

pub mod hooks {
    use std::collections::HashMap;

    /// Config for a single hook entry as it appears in the Styx config file.
    /// The hook name comes from the map key.
    #[derive(Debug, Clone, PartialEq, Eq, facet::Facet)]
    pub struct HookEntryConfig {
        pub command: String,
        #[facet(default)]
        pub cwd: Option<String>,
        #[facet(default)]
        pub glob: Vec<String>,
    }

    /// Raw hooks config as parsed from Styx. Each hook point maps names to configs.
    #[derive(Debug, Clone, Default, PartialEq, Eq, facet::Facet)]
    pub struct HooksConfig {
        #[facet(default)]
        pub worktree_setup: HashMap<String, HookEntryConfig>,
        #[facet(default)]
        pub pre_commit: HashMap<String, HookEntryConfig>,
        #[facet(default)]
        pub checks: HashMap<String, HookEntryConfig>,
    }

    /// A resolved hook definition with its name (from the map key).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HookDef {
        pub name: String,
        pub command: String,
        pub cwd: Option<String>,
        pub glob: Vec<String>,
    }

    /// Resolved hooks with ordered Vec<HookDef> for each hook point.
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ResolvedHooks {
        pub worktree_setup: Vec<HookDef>,
        pub pre_commit: Vec<HookDef>,
        pub checks: Vec<HookDef>,
    }

    fn resolve_map(map: HashMap<String, HookEntryConfig>) -> Vec<HookDef> {
        let mut hooks: Vec<HookDef> = map
            .into_iter()
            .map(|(name, entry)| HookDef {
                name,
                command: entry.command,
                cwd: entry.cwd,
                glob: entry.glob,
            })
            .collect();
        hooks.sort_by(|a, b| a.name.cmp(&b.name));
        hooks
    }

    impl HooksConfig {
        /// Convert the map-based config into ordered, resolved hook definitions.
        pub fn resolve(self) -> ResolvedHooks {
            ResolvedHooks {
                worktree_setup: resolve_map(self.worktree_setup),
                pre_commit: resolve_map(self.pre_commit),
                checks: resolve_map(self.checks),
            }
        }
    }

    /// Root project config file structure — lives at `.config/ship/config.styx`.
    #[derive(Debug, Clone, Default, facet::Facet)]
    pub struct ProjectConfig {
        #[facet(default)]
        pub hooks: HooksConfig,
    }
}

pub mod persistence {
    use crate::agent::{AgentKind, AgentSnapshot, Role};
    use crate::events::{ContentBlock, SessionEventEnvelope};
    use crate::ids::{BlockId, ProjectName, SessionId};
    use crate::protocol::{AutonomyMode, McpServerConfig};
    use crate::session::SessionStartupState;
    use crate::task::TaskRecord;

    #[derive(Debug, Clone, facet::Facet)]
    pub struct SessionConfig {
        pub project: ProjectName,
        pub base_branch: String,
        pub branch_name: String,
        pub captain_kind: AgentKind,
        pub mate_kind: AgentKind,
        #[facet(default)]
        pub captain_preset_id: Option<crate::agent::AgentPresetId>,
        #[facet(default)]
        pub mate_preset_id: Option<crate::agent::AgentPresetId>,
        #[facet(default)]
        pub captain_provider: Option<crate::agent::AgentProviderId>,
        #[facet(default)]
        pub mate_provider: Option<crate::agent::AgentProviderId>,
        #[facet(default)]
        pub captain_model_id: Option<String>,
        #[facet(default)]
        pub mate_model_id: Option<String>,
        pub autonomy_mode: AutonomyMode,
        pub mcp_servers: Vec<McpServerConfig>,
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
        pub pending_mate_guidance: Option<String>,
        pub content_history: Vec<TaskContentRecord>,
        // r[backend.persistence-contents]
        pub event_log: Vec<SessionEventEnvelope>,
    }

    // r[session.persistent]
    #[derive(Debug, Clone, facet::Facet)]
    pub struct PersistedSession {
        pub id: SessionId,
        #[facet(default)]
        pub created_at: String,
        pub config: SessionConfig,
        pub captain: AgentSnapshot,
        pub mate: AgentSnapshot,
        pub startup_state: SessionStartupState,
        pub session_event_log: Vec<SessionEventEnvelope>,
        pub current_task: Option<CurrentTask>,
        pub task_history: Vec<TaskRecord>,
        #[facet(default)]
        pub title: Option<String>,
        // r[proto.archive-session]
        #[facet(default)]
        pub archived_at: Option<String>,
        /// ACP session ID for the captain agent (used for session resume)
        #[facet(default)]
        pub captain_acp_session_id: Option<String>,
        /// ACP session ID for the mate agent (used for session resume)
        #[facet(default)]
        pub mate_acp_session_id: Option<String>,
    }
}

pub use agent::{
    AgentAcpInfo, AgentKind, AgentPreset, AgentPresetId, AgentPresetsConfig, AgentProviderId,
    AgentSnapshot, AgentState, AssignFileRef, CaptainAssignExtras, DirtySessionStrategy,
    EffortValue, PermissionRequest, PlanStep, PlanStepInput, PlanStepStatus, Role,
};
pub use events::{
    ActivityEntry, ActivityKind, BlockPatch, CommitSummary, ContentBlock, GlobalEvent,
    HookCheckResult, PermissionResolution, SessionEvent, SessionEventEnvelope, SubscribeMessage,
    TaskRecapStats, TextSource, ToolCallContent, ToolCallLocation, ToolCallStatus,
    WorkflowMilestoneKind,
};
pub use hooks::{HookDef, HookEntryConfig, HooksConfig, ProjectConfig, ResolvedHooks};
pub use ids::{BlockId, ProjectName, SessionId, TaskId};
pub use persistence::{CurrentTask, PersistedSession, SessionConfig, TaskContentRecord};
pub use prompt::PromptContentPart;
pub use protocol::{
    AgentDiscovery, ArchiveSessionRequest, ArchiveSessionResponse, AutonomyMode, CaptainGitStatus,
    CaptainRebaseAction, CaptainRebaseActionOutcome, CaptainRebaseActionResult,
    CaptainRebaseStatus, CaptainReviewDiff, CaptainReviewDiffState, CloseSessionRequest,
    CloseSessionResponse, CreateSessionRequest, CreateSessionResponse, HumanReviewRequest,
    McpDiffContent, McpEnvVar, McpHeader, McpHttpServerConfig, McpServerConfig, McpSseServerConfig,
    McpStdioServerConfig, McpToolCallResponse, NewSessionDefaults, ProjectInfo, ServerInfo,
    SessionDetail, SessionSummary, SetAgentEffortResponse, SetAgentModelResponse,
    SetAgentPresetResponse, WorktreeDiffStats,
};
pub use session::{SessionStartupStage, SessionStartupState};
pub use structured::{
    JsonEntry, JsonValue, PermissionOption, PermissionOptionKind, TerminalExit, TerminalSnapshot,
    ToolCallError, ToolCallKind, ToolTarget,
};
pub use task::{TaskRecord, TaskStatus};
pub use transcription::{TranscribeMessage, TranscribeSegment};
