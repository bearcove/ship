use std::future::Future;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rust_mcp_sdk::McpServer;
use rust_mcp_sdk::mcp_server::hyper_runtime::HyperRuntime;
use rust_mcp_sdk::mcp_server::{
    HyperServerOptions, ServerHandler, ToMcpServerHandler, hyper_server,
};
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, Implementation, InitializeResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, RpcError, ServerCapabilities, ServerCapabilitiesTools,
    TextContent, Tool, ToolInputSchema, schema_utils::CallToolError,
};
use serde_json::Value;

pub type ToolHandler =
    Arc<dyn Fn(String, Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> + Send + Sync>;

#[derive(Clone)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub struct ToolResult {
    pub text: String,
    pub is_error: bool,
}

pub struct CaptainMcpServerHandle {
    url: String,
    runtime: HyperRuntime,
}

impl CaptainMcpServerHandle {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn shutdown(&self) {
        self.runtime.graceful_shutdown(Some(Duration::from_secs(1)));
    }
}

struct CaptainMcpHandler {
    tools: Arc<Vec<ToolDefinition>>,
    tool_handler: ToolHandler,
}

#[async_trait]
impl ServerHandler for CaptainMcpHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: self.tools.iter().map(to_sdk_tool).collect(),
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        let arguments = params.arguments.map(Value::Object).unwrap_or(Value::Null);
        let result = (self.tool_handler)(params.name, arguments).await;
        Ok(tool_result_to_sdk(result))
    }
}

pub async fn start_server(
    tools: Vec<ToolDefinition>,
    tool_handler: ToolHandler,
) -> Result<CaptainMcpServerHandle, String> {
    let port = reserve_loopback_port()?;
    let url = format!("http://127.0.0.1:{port}/mcp");
    let server = hyper_server::create_server(
        server_details(),
        CaptainMcpHandler {
            tools: Arc::new(tools),
            tool_handler,
        }
        .to_mcp_server_handler(),
        HyperServerOptions {
            host: "127.0.0.1".to_owned(),
            port,
            custom_streamable_http_endpoint: Some("/mcp".to_owned()),
            enable_json_response: Some(true),
            sse_support: false,
            ..Default::default()
        },
    );
    let runtime = server
        .start_runtime()
        .await
        .map_err(|error| format!("failed to start captain MCP server: {error}"))?;
    Ok(CaptainMcpServerHandle { url, runtime })
}

fn server_details() -> InitializeResult {
    InitializeResult {
        server_info: Implementation {
            name: "ship".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            title: Some("Ship".to_owned()),
            description: Some("Ship captain MCP server".to_owned()),
            icons: Vec::new(),
            website_url: None,
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools {
                list_changed: Some(false),
            }),
            ..Default::default()
        },
        instructions: None,
        meta: None,
        protocol_version: ProtocolVersion::V2025_11_25.into(),
    }
}

fn to_sdk_tool(tool: &ToolDefinition) -> Tool {
    Tool {
        annotations: None,
        description: Some(tool.description.to_owned()),
        execution: None,
        icons: Vec::new(),
        input_schema: serde_json::from_value::<ToolInputSchema>(tool.input_schema.clone())
            .expect("tool schema should be a valid MCP input schema"),
        meta: None,
        name: tool.name.to_owned(),
        output_schema: None,
        title: None,
    }
}

fn tool_result_to_sdk(result: ToolResult) -> CallToolResult {
    CallToolResult {
        content: vec![TextContent::from(result.text).into()],
        is_error: result.is_error.then_some(true),
        meta: None,
        structured_content: None,
    }
}

fn reserve_loopback_port() -> Result<u16, String> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("failed to reserve captain MCP port: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("failed to read reserved captain MCP port: {error}"))?
        .port();
    drop(listener);
    Ok(port)
}
