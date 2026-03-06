use roam::Tx;
use ship_types::{
    AgentDiscovery, AssignTaskResponse, CloseSessionRequest, CloseSessionResponse,
    CreateSessionRequest, CreateSessionResponse, McpToolCallResponse, ProjectInfo, ProjectName,
    Role, SessionDetail, SessionId, SessionSummary, SubscribeMessage,
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

    // r[proto.assign]
    async fn assign(&self, session: SessionId, description: String) -> AssignTaskResponse;

    // r[proto.steer]
    async fn steer(&self, session: SessionId, content: String);

    // r[acp.prompt]
    async fn prompt_captain(&self, session: SessionId, content: String);

    // r[proto.accept]
    async fn accept(&self, session: SessionId);

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

    // r[proto.close-session]
    async fn close_session(&self, req: CloseSessionRequest) -> CloseSessionResponse;

    // r[event.subscribe.roam-channel]
    async fn subscribe_events(&self, session: SessionId, output: Tx<SubscribeMessage>);
}

// r[captain.tool.implementation]
#[roam::service]
pub trait CaptainMcp {
    // r[captain.tool.assign]
    async fn captain_assign(&self, description: String) -> McpToolCallResponse;

    // r[captain.tool.steer]
    async fn captain_steer(&self, message: String) -> McpToolCallResponse;

    // r[captain.tool.accept]
    async fn captain_accept(&self, summary: Option<String>) -> McpToolCallResponse;

    // r[captain.tool.cancel]
    async fn captain_cancel(&self, reason: Option<String>) -> McpToolCallResponse;

    // r[captain.tool.notify-human]
    async fn captain_notify_human(&self, message: String) -> McpToolCallResponse;
}

// r[mate.tool.implementation]
#[roam::service]
pub trait MateMcp {
    // r[mate.tool.send-update]
    async fn mate_send_update(&self, message: String) -> McpToolCallResponse;

    // r[mate.tool.ask-captain]
    async fn mate_ask_captain(&self, question: String) -> McpToolCallResponse;

    // r[mate.tool.submit]
    async fn mate_submit(&self, summary: String) -> McpToolCallResponse;
}
