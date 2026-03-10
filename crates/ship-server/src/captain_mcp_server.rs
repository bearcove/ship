use std::sync::Arc;

use async_trait::async_trait;
use roam::{ConnectionSettings, MetadataEntry, MetadataFlags, MetadataValue, NoopCaller, Parity};
use rust_mcp_sdk::mcp_server::{McpServerOptions, ServerHandler, server_runtime};
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, Implementation, InitializeResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, RpcError, ServerCapabilities, ServerCapabilitiesTools,
    TextContent, Tool, ToolInputSchema, schema_utils::CallToolError,
};
use rust_mcp_sdk::{McpServer, StdioTransport, ToMcpServerHandler, TransportOptions};
use serde_json::{Value, json};
use ship_service::CaptainMcpClient;
use ship_types::SessionId;

pub struct CaptainMcpServerArgs {
    pub session_id: SessionId,
    pub server_ws_url: String,
}

#[derive(Clone)]
struct ToolDefinition {
    name: &'static str,
    description: &'static str,
    input_schema: Value,
}

#[derive(Clone)]
struct CaptainMcpHandler {
    client: CaptainMcpClient,
    tools: Arc<Vec<ToolDefinition>>,
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
        let result = match params.name.as_str() {
            // r[captain.tool.assign]
            "captain_assign" => {
                let Some(title) = arguments.get("title").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: title", true));
                };
                let Some(description) = arguments.get("description").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: description", true));
                };
                let keep = arguments
                    .get("keep")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.client
                    .captain_assign(title.to_owned(), description.to_owned(), keep)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.steer]
            "captain_steer" => {
                let Some(message) = arguments.get("message").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: message", true));
                };
                self.client
                    .captain_steer(message.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.accept]
            "captain_accept" => {
                let summary = arguments
                    .get("summary")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.client
                    .captain_accept(summary)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.cancel]
            "captain_cancel" => {
                let reason = arguments
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.client
                    .captain_cancel(reason)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.notify-human]
            "captain_notify_human" => {
                let Some(message) = arguments.get("message").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: message", true));
                };
                self.client
                    .captain_notify_human(message.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            other => return Err(CallToolError::unknown_tool(other.to_owned())),
        };

        Ok(tool_result(&result.text, result.is_error))
    }
}

pub async fn run_stdio_server(args: CaptainMcpServerArgs) -> Result<(), String> {
    let ws_stream = tokio_tungstenite::connect_async(&args.server_ws_url)
        .await
        .map_err(|error| format!("failed to connect to ship server websocket: {error}"))?
        .0;
    let link = roam_websocket::WsLink::new(ws_stream);
    let (_root_guard, session_handle) = roam::initiator(link)
        .establish::<NoopCaller>(())
        .await
        .map_err(|error| format!("failed to establish roam session: {error:?}"))?;

    let connection = session_handle
        .open_connection(
            ConnectionSettings {
                parity: Parity::Odd,
                max_concurrent_requests: 64,
            },
            vec![
                metadata_string("ship-service", "captain-mcp"),
                metadata_string_owned("ship-session-id", args.session_id.0.clone()),
            ],
        )
        .await
        .map_err(|error| format!("failed to open captain MCP connection: {error:?}"))?;

    let mut driver = roam::Driver::new(connection, ());
    let client = CaptainMcpClient::from(driver.caller());
    let _driver_task = tokio::spawn(async move {
        driver.run().await;
    });

    let transport = StdioTransport::new(TransportOptions::default())
        .map_err(|error| format!("failed to create stdio transport: {error}"))?;
    let server = server_runtime::create_server(McpServerOptions {
        server_details: server_details(),
        transport,
        handler: CaptainMcpHandler {
            client,
            tools: Arc::new(tool_definitions()),
        }
        .to_mcp_server_handler(),
        task_store: None,
        client_task_store: None,
    });

    server
        .start()
        .await
        .map_err(|error| format!("captain MCP server failed: {error}"))?;
    Ok(())
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

fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "captain_assign",
            description: "Assign a task to the mate. The mate will start working on it immediately. Set keep=true to reuse the mate's existing context; omit or set false to restart the mate with a fresh context (default).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Short title for the task (under 60 chars). Shown in the UI sidebar and headers." },
                    "description": { "type": "string", "description": "Full task description with all details the mate needs." },
                    "keep": { "type": "boolean" }
                },
                "required": ["title", "description"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_steer",
            description: "Send direction to the mate on the current task. Fire-and-forget: returns immediately.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_accept",
            description: "Accept the mate's submitted work. Only valid after the mate calls mate_submit.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" }
                },
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_cancel",
            description: "Cancel the current task.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "reason": { "type": "string" }
                },
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_notify_human",
            description: "Ask the human for guidance. Blocks until the human responds.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"],
                "additionalProperties": false,
            }),
        },
    ]
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

fn tool_result(text: &str, is_error: bool) -> CallToolResult {
    CallToolResult {
        content: vec![TextContent::from(text.to_owned()).into()],
        is_error: is_error.then_some(true),
        meta: None,
        structured_content: None,
    }
}

fn call_tool_rpc_error(error: impl std::fmt::Debug) -> CallToolError {
    CallToolError::from_message(format!("captain MCP RPC failed: {error:?}"))
}

fn metadata_string<'a>(key: &'a str, value: &'a str) -> MetadataEntry<'a> {
    MetadataEntry {
        key,
        value: MetadataValue::String(value),
        flags: MetadataFlags::NONE,
    }
}

fn metadata_string_owned(key: &'static str, value: String) -> MetadataEntry<'static> {
    MetadataEntry {
        key,
        value: MetadataValue::String(Box::leak(value.into_boxed_str())),
        flags: MetadataFlags::NONE,
    }
}
