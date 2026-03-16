use facet::Facet;
use facet_mcp::{McpServer, McpServerInfo, ToolCtx, ToolError};
use ship_service::AdmiralMcpClient;
use ship_types::SessionId;

// ── Result type shared by most admiral tools ────────────────────────

#[derive(Debug, Facet)]
pub struct TextResult {
    /// The response text.
    pub text: String,
}

// ── admiral_list_lanes ──────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct ListLanesArgs {}

facet_mcp::tool! {
    /// List all active sessions (lanes). Returns a summary of each session
    /// including ID, slug, title, task status, and project.
    pub(crate) async fn admiral_list_lanes(args: ListLanesArgs, ctx: &ToolCtx) -> TextResult {
        let _ = args;
        let client = ctx.get::<AdmiralMcpClient>();
        let resp = client.admiral_list_lanes().await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── admiral_create_lane ─────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CreateLaneArgs {
    /// Project name to create the lane for.
    project: String,
    /// Initial task description to send to the captain.
    description: String,
}

facet_mcp::tool! {
    /// Create a new session (lane) for a project. Uses Claude for both captain
    /// and mate agents. The captain will be bootstrapped and ready for work.
    pub(crate) async fn admiral_create_lane(args: CreateLaneArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<AdmiralMcpClient>();
        let resp = client
            .admiral_create_lane(args.project, args.description)
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── admiral_steer_captain ───────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct SteerCaptainArgs {
    /// Session ID (slug) to steer.
    session_id: String,
    /// Message to send to the captain.
    message: String,
}

facet_mcp::tool! {
    /// Send a message to a captain in a specific session. Fire-and-forget: returns immediately.
    pub(crate) async fn admiral_steer_captain(args: SteerCaptainArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<AdmiralMcpClient>();
        let resp = client
            .admiral_steer_captain(SessionId(args.session_id), args.message)
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── admiral_post_to_human ───────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct PostToHumanArgs {
    /// Message to show the human.
    message: String,
}

facet_mcp::tool! {
    /// Post a message to the human via the activity log. Use this to surface
    /// important information, status updates, or decisions that need human attention.
    pub(crate) async fn admiral_post_to_human(args: PostToHumanArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<AdmiralMcpClient>();
        let resp = client
            .admiral_post_to_human(args.message)
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── admiral_list_projects ───────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct ListProjectsArgs {}

facet_mcp::tool! {
    /// List all registered projects with their paths and validity status.
    pub(crate) async fn admiral_list_projects(args: ListProjectsArgs, ctx: &ToolCtx) -> TextResult {
        let _ = args;
        let client = ctx.get::<AdmiralMcpClient>();
        let resp = client.admiral_list_projects().await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── read_file ───────────────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct ReadFileArgs {
    /// Absolute file path.
    path: String,
    /// 1-based line to start from.
    #[facet(default)]
    offset: Option<u64>,
    /// Maximum number of lines to return.
    #[facet(default)]
    limit: Option<u64>,
}

facet_mcp::tool! {
    /// Read a file by absolute path. Returns numbered lines. Use offset/limit to page through large files.
    pub(crate) async fn read_file(args: ReadFileArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<AdmiralMcpClient>();
        let resp = client
            .admiral_read_file(args.path, args.offset, args.limit)
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── run_command ─────────────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct RunCommandArgs {
    /// Shell command to run.
    command: String,
    /// Absolute path to run the command in (optional).
    #[facet(default)]
    cwd: Option<String>,
}

facet_mcp::tool! {
    /// Run a shell command via sh -c. Use rg instead of grep and fd instead of find.
    /// The admiral has no worktree — pass an absolute path via cwd if you need to run in a specific directory.
    pub(crate) async fn run_command(args: RunCommandArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<AdmiralMcpClient>();
        let resp = client
            .admiral_run_command(args.command, args.cwd)
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── Server builder ──────────────────────────────────────────────────

pub fn admiral_server(ctx: ToolCtx) -> McpServer {
    McpServer::new(
        ctx,
        McpServerInfo {
            name: "ship".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
    )
    .tool::<admiral_list_lanes>()
    .tool::<admiral_create_lane>()
    .tool::<admiral_steer_captain>()
    .tool::<admiral_post_to_human>()
    .tool::<admiral_list_projects>()
    .tool::<read_file>()
    .tool::<run_command>()
    .tool::<super::shared::web_search>()
}

fn rpc_err(e: impl std::fmt::Debug) -> ToolError {
    ToolError::new(format!("{e:?}"))
}

fn rpc_result(resp: ship_types::McpToolCallResponse) -> Result<TextResult, ToolError> {
    if resp.is_error {
        Err(ToolError::new(resp.text))
    } else {
        Ok(TextResult { text: resp.text })
    }
}
