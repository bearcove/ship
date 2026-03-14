use std::sync::Arc;

use super::worktree_tools::{ToolDefinition, to_sdk_tool, web_search_tool};
use async_trait::async_trait;
use roam::{ConnectionSettings, MetadataEntry, MetadataFlags, MetadataValue, NoopCaller, Parity};
use rust_mcp_sdk::mcp_server::{McpServerOptions, ServerHandler, server_runtime};
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, Implementation, InitializeResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, RpcError, ServerCapabilities, ServerCapabilitiesTools,
    TextContent, schema_utils::CallToolError,
};
use rust_mcp_sdk::{McpServer, StdioTransport, ToMcpServerHandler, TransportOptions};
use serde_json::{Value, json};
use ship_service::AdmiralMcpClient;
use ship_types::SessionId;

pub struct AdmiralMcpServerArgs {
    pub session_id: SessionId,
    pub server_ws_url: String,
}

#[derive(Clone)]
struct AdmiralMcpHandler {
    client: AdmiralMcpClient,
    tools: Arc<Vec<ToolDefinition>>,
    kagi_api_key: Option<String>,
    http_client: reqwest::Client,
}

#[async_trait]
impl ServerHandler for AdmiralMcpHandler {
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
            // r[admiral.tool.list-lanes]
            "admiral_list_lanes" => self
                .client
                .admiral_list_lanes()
                .await
                .map_err(call_tool_rpc_error)?,
            // r[admiral.tool.create-lane]
            "admiral_create_lane" => {
                let Some(project) = arguments.get("project").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: project", true));
                };
                let Some(description) = arguments.get("description").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: description", true));
                };
                self.client
                    .admiral_create_lane(project.to_owned(), description.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[admiral.tool.steer-captain]
            "admiral_steer_captain" => {
                let Some(session_id) = arguments.get("session_id").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: session_id", true));
                };
                let Some(message) = arguments.get("message").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: message", true));
                };
                self.client
                    .admiral_steer_captain(SessionId(session_id.to_owned()), message.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[admiral.tool.post-to-human]
            "admiral_post_to_human" => {
                let Some(message) = arguments.get("message").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: message", true));
                };
                self.client
                    .admiral_post_to_human(message.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[admiral.tool.list-projects]
            "admiral_list_projects" => self
                .client
                .admiral_list_projects()
                .await
                .map_err(call_tool_rpc_error)?,
            // r[admiral.tool.read-file]
            "read_file" => {
                let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: path", true));
                };
                let offset = arguments.get("offset").and_then(Value::as_u64);
                let limit = arguments.get("limit").and_then(Value::as_u64);
                self.client
                    .admiral_read_file(path.to_owned(), offset, limit)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[admiral.tool.run-command]
            "run_command" => {
                let Some(command) = arguments.get("command").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: command", true));
                };
                let cwd = arguments
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.client
                    .admiral_run_command(command.to_owned(), cwd)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            "web_search" => {
                let Some(query) = arguments.get("query").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: query", true));
                };
                let Some(ref api_key) = self.kagi_api_key else {
                    return Ok(tool_result("KAGI_API_KEY is not configured", true));
                };
                return Ok(kagi_web_search(&self.http_client, api_key, query).await);
            }
            other => return Err(CallToolError::unknown_tool(other.to_owned())),
        };

        Ok(tool_result(&result.text, result.is_error))
    }
}

pub async fn run_stdio_server(args: AdmiralMcpServerArgs) -> Result<(), String> {
    let kagi_api_key = match std::env::var("KAGI_API_KEY") {
        Ok(key) => Some(key),
        Err(_) => {
            tracing::warn!("KAGI_API_KEY is not set; web_search tool will be unavailable");
            None
        }
    };

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
                metadata_string("ship-service", "admiral-mcp"),
                metadata_string_owned("ship-session-id", args.session_id.0.clone()),
            ],
        )
        .await
        .map_err(|error| format!("failed to open admiral MCP connection: {error:?}"))?;

    let mut driver = roam::Driver::new(connection, ());
    let client = AdmiralMcpClient::from(driver.caller());
    let _driver_task = tokio::spawn(async move {
        driver.run().await;
    });

    let transport = StdioTransport::new(TransportOptions::default())
        .map_err(|error| format!("failed to create stdio transport: {error}"))?;
    let server = server_runtime::create_server(McpServerOptions {
        server_details: server_details(),
        transport,
        handler: AdmiralMcpHandler {
            client,
            tools: Arc::new(tool_definitions()),
            kagi_api_key,
            http_client: reqwest::Client::new(),
        }
        .to_mcp_server_handler(),
        task_store: None,
        client_task_store: None,
    });

    server
        .start()
        .await
        .map_err(|error| format!("admiral MCP server failed: {error}"))?;
    Ok(())
}

fn server_details() -> InitializeResult {
    InitializeResult {
        server_info: Implementation {
            name: "ship".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            title: Some("Ship".to_owned()),
            description: Some("Ship admiral MCP server".to_owned()),
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
            name: "admiral_list_lanes",
            description: "List all active sessions (lanes). Returns a summary of each session including ID, slug, title, task status, and project.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "admiral_create_lane",
            description: "Create a new session (lane) for a project. Uses Claude for both captain and mate agents. The captain will be bootstrapped and ready for work.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name to create the lane for." },
                    "description": { "type": "string", "description": "Initial task description to send to the captain." }
                },
                "required": ["project", "description"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "admiral_steer_captain",
            description: "Send a message to a captain in a specific session. Fire-and-forget: returns immediately.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID (slug) to steer." },
                    "message": { "type": "string", "description": "Message to send to the captain." }
                },
                "required": ["session_id", "message"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "admiral_post_to_human",
            description: "Post a message to the human via the activity log. Use this to surface important information, status updates, or decisions that need human attention.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Message to show the human." }
                },
                "required": ["message"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "admiral_list_projects",
            description: "List all registered projects with their paths and validity status.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "read_file",
            description: "Read a file by absolute path. Returns numbered lines. Use offset/limit to page through large files.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute file path." },
                    "offset": { "type": "integer", "minimum": 1, "description": "1-based line to start from." },
                    "limit": { "type": "integer", "minimum": 1, "description": "Maximum number of lines to return." }
                },
                "required": ["path"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "run_command",
            description: "Run a shell command via sh -c. Use rg instead of grep and fd instead of find. \
The admiral has no worktree — pass an absolute path via cwd if you need to run in a specific directory.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "cwd": { "type": "string", "description": "Absolute path to run the command in (optional)." }
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        },
        web_search_tool(),
    ]
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
    CallToolError::from_message(format!("admiral MCP RPC failed: {error:?}"))
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

async fn kagi_web_search(
    http_client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> CallToolResult {
    let response = match http_client
        .post("https://kagi.com/api/v0/fastgpt")
        .header("Authorization", format!("Bot {api_key}"))
        .json(&json!({ "query": query }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(error) => return tool_result(&format!("web_search request failed: {error}"), true),
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return tool_result(
            &format!("web_search request failed with status {status}: {body}"),
            true,
        );
    }

    let body: Value = match response.json().await {
        Ok(v) => v,
        Err(error) => {
            return tool_result(
                &format!("failed to parse web_search response: {error}"),
                true,
            );
        }
    };

    let output = body
        .get("data")
        .and_then(|d| d.get("output"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let mut text = output.to_owned();

    if let Some(refs) = body
        .get("data")
        .and_then(|d| d.get("references"))
        .and_then(Value::as_array)
        && !refs.is_empty() {
            text.push_str("\n\n## References\n");
            for r in refs {
                let title = r.get("title").and_then(Value::as_str).unwrap_or("Untitled");
                let url = r.get("url").and_then(Value::as_str).unwrap_or("");
                let snippet = r.get("snippet").and_then(Value::as_str).unwrap_or("");
                if snippet.is_empty() {
                    text.push_str(&format!("- [{title}]({url})\n"));
                } else {
                    text.push_str(&format!("- [{title}]({url}): {snippet}\n"));
                }
            }
        }

    tool_result(&text, false)
}
