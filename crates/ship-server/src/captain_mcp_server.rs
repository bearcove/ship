use facet_mcp::ToolCtx;
use ship_mcp::KagiApiKey;
use ship_service::CaptainMcpClient;
use ship_types::SessionId;

pub struct CaptainMcpServerArgs {
    pub session_id: SessionId,
    pub server_ws_url: String,
}

pub async fn run_stdio_server(args: CaptainMcpServerArgs) -> Result<(), String> {
    let (caller, _root_guard, _driver_task) =
        ship_mcp::connect_to_ship(&args.server_ws_url, "captain-mcp", &args.session_id.0)
            .await
            .map_err(|e| e.to_string())?;

    let client = CaptainMcpClient::from(caller);

    let mut ctx = ToolCtx::new();
    ctx.insert(client);
    ctx.insert(reqwest::Client::new());
    if let Ok(key) = std::env::var("KAGI_API_KEY") {
        ctx.insert(KagiApiKey(key));
    } else {
        tracing::warn!("KAGI_API_KEY is not set; web_search tool will be unavailable");
    }

    ship_mcp::captain_server(ctx)
        .run()
        .await
        .map_err(|e| format!("captain MCP server failed: {e}"))
}
