use roam::Tx;
use ship_types::{
    AgentDiscovery, CloseSessionRequest, CloseSessionResponse, CreateSessionRequest,
    CreateSessionResponse, McpToolCallResponse, ProjectInfo, ProjectName, PromptContentPart, Role,
    SessionDetail, SessionId, SessionSummary, SetAgentEffortResponse, SetAgentModelResponse,
    SubscribeMessage,
};

// r[backend.rpc]
#[roam::service]
pub trait Ship {
    // r[proto.list-projects]
    async fn list_projects(&self) -> Vec<ProjectInfo>;

    // r[proto.add-project]
    async fn add_project(&self, path: String) -> ProjectInfo;

    // r[proto.list-branches]
    async fn list_branches(&self, project: ProjectName) -> Vec<String>;

    // r[proto.list-sessions]
    async fn list_sessions(&self) -> Vec<SessionSummary>;

    // r[server.agent-discovery]
    async fn agent_discovery(&self) -> AgentDiscovery;

    // r[proto.get-session]
    async fn get_session(&self, id: SessionId) -> SessionDetail;

    // r[proto.create-session]
    async fn create_session(&self, req: CreateSessionRequest) -> CreateSessionResponse;

    // r[proto.steer]
    async fn steer(&self, session: SessionId, parts: Vec<PromptContentPart>);

    // r[acp.prompt]
    async fn prompt_captain(&self, session: SessionId, parts: Vec<PromptContentPart>);

    // r[proto.accept]
    async fn accept(&self, session: SessionId);

    // r[proto.reply-to-human]
    async fn reply_to_human(&self, session: SessionId, message: String);

    // r[proto.cancel]
    async fn cancel(&self, session: SessionId);

    // r[proto.resolve-permission]
    async fn resolve_permission(
        &self,
        session: SessionId,
        permission_id: String,
        option_id: String,
    );

    // r[proto.retry-agent]
    async fn retry_agent(&self, session: SessionId, role: Role);

    async fn set_agent_model(
        &self,
        session: SessionId,
        role: Role,
        model_id: String,
    ) -> SetAgentModelResponse;

    // r[proto.set-agent-effort]
    async fn set_agent_effort(
        &self,
        session: SessionId,
        role: Role,
        config_id: String,
        value_id: String,
    ) -> SetAgentEffortResponse;

    // r[proto.close-session]
    async fn close_session(&self, req: CloseSessionRequest) -> CloseSessionResponse;

    // r[ui.composer.file-mention]
    async fn list_worktree_files(&self, session: SessionId) -> Vec<String>;

    // r[event.subscribe.roam-channel]
    async fn subscribe_events(&self, session: SessionId, output: Tx<SubscribeMessage>);
}

// r[captain.tool.implementation]
#[roam::service]
pub trait CaptainMcp {
    // r[captain.tool.assign]
    async fn captain_assign(
        &self,
        title: String,
        description: String,
        keep: bool,
    ) -> McpToolCallResponse;

    // r[captain.tool.steer]
    async fn captain_steer(&self, message: String) -> McpToolCallResponse;

    // r[captain.tool.accept]
    async fn captain_accept(&self, summary: Option<String>) -> McpToolCallResponse;

    // r[captain.tool.cancel]
    async fn captain_cancel(&self, reason: Option<String>) -> McpToolCallResponse;

    // r[captain.tool.notify-human]
    async fn captain_notify_human(&self, message: String) -> McpToolCallResponse;

    // r[captain.tool.read-only]
    async fn captain_read_file(
        &self,
        path: String,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> McpToolCallResponse;

    // r[captain.tool.read-only]
    async fn captain_search_files(&self, args: String) -> McpToolCallResponse;

    // r[captain.tool.read-only]
    async fn captain_list_files(&self, args: String) -> McpToolCallResponse;
}

// r[mate.tool.implementation]
#[roam::service]
pub trait MateMcp {
    // r[mate.tool.run-command]
    async fn run_command(&self, command: String, cwd: Option<String>) -> McpToolCallResponse;

    // r[mate.tool.read-file]
    async fn read_file(
        &self,
        path: String,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> McpToolCallResponse;

    // r[mate.tool.write-file]
    async fn write_file(&self, path: String, content: String) -> McpToolCallResponse;

    // r[mate.tool.edit-prepare]
    async fn edit_prepare(
        &self,
        path: String,
        old_string: String,
        new_string: String,
        replace_all: Option<bool>,
    ) -> McpToolCallResponse;

    // r[mate.tool.edit-confirm]
    async fn edit_confirm(&self, edit_id: String) -> McpToolCallResponse;

    // r[mate.tool.search-files]
    async fn search_files(&self, args: String) -> McpToolCallResponse;

    // r[mate.tool.list-files]
    async fn list_files(&self, args: String) -> McpToolCallResponse;

    // r[mate.tool.send-update]
    async fn mate_send_update(&self, message: String) -> McpToolCallResponse;

    // r[mate.tool.plan-create]
    async fn set_plan(&self, steps: Vec<String>) -> McpToolCallResponse;

    // r[mate.tool.plan-step-complete]
    async fn plan_step_complete(&self, step_index: u64, summary: String) -> McpToolCallResponse;

    // r[mate.tool.cargo-check]
    async fn cargo_check(&self, args: Option<String>) -> McpToolCallResponse;

    // r[mate.tool.cargo-clippy]
    async fn cargo_clippy(&self, args: Option<String>) -> McpToolCallResponse;

    // r[mate.tool.cargo-test]
    async fn cargo_test(&self, args: Option<String>) -> McpToolCallResponse;

    // r[mate.tool.pnpm-install]
    async fn pnpm_install(&self, args: Option<String>) -> McpToolCallResponse;

    // r[mate.tool.ask-captain]
    async fn mate_ask_captain(&self, question: String) -> McpToolCallResponse;

    // r[mate.tool.submit]
    async fn mate_submit(&self, summary: String) -> McpToolCallResponse;
}
