use std::sync::Arc;

use super::worktree_tools::{ToolDefinition, code_tool, to_sdk_tool, web_search_tool};
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
use ship_service::MateMcpClient;
use ship_types::{McpToolCallResponse, PlanStepInput, SessionId};

pub struct MateMcpServerArgs {
    pub session_id: SessionId,
    pub server_ws_url: String,
}

#[derive(Clone)]
struct MateMcpHandler {
    client: MateMcpClient,
    tools: Arc<Vec<ToolDefinition>>,
    kagi_api_key: Option<String>,
    http_client: reqwest::Client,
}

#[async_trait]
impl ServerHandler for MateMcpHandler {
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
            // r[mate.tool.plan-create]
            "set_plan" => {
                let Some(steps) = arguments.get("steps").and_then(Value::as_array) else {
                    return Ok(tool_result("missing required argument: steps", true));
                };
                let steps = steps
                    .iter()
                    .map(|value| {
                        let title = value.get("title").and_then(Value::as_str)?.to_owned();
                        let description =
                            value.get("description").and_then(Value::as_str)?.to_owned();
                        Some(PlanStepInput { title, description })
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        call_tool_rpc_error(
                            "each step must be an object with title and description",
                        )
                    })?;
                self.client
                    .set_plan(steps)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.ask-captain]
            "mate_ask_captain" => {
                let Some(question) = arguments.get("question").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: question", true));
                };
                self.client
                    .mate_ask_captain(question.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.code]
            "code" => {
                let Some(ops) = arguments.get("ops").and_then(Value::as_array) else {
                    return Ok(tool_result("missing required argument: ops", true));
                };
                let ops_json = serde_json::to_string(ops)
                    .map_err(|e| call_tool_rpc_error(format!("failed to serialize ops: {e}")))?;
                self.client
                    .code(ops_json)
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

        Ok(mcp_tool_call_result(&result))
    }
}

pub async fn run_stdio_server(args: MateMcpServerArgs) -> Result<(), String> {
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
                metadata_string("ship-service", "mate-mcp"),
                metadata_string_owned("ship-session-id", args.session_id.0.clone()),
            ],
        )
        .await
        .map_err(|error| format!("failed to open mate MCP connection: {error:?}"))?;

    let mut driver = roam::Driver::new(connection, ());
    let client = MateMcpClient::from(driver.caller());
    let _driver_task = tokio::spawn(async move {
        driver.run().await;
    });

    let transport = StdioTransport::new(TransportOptions::default())
        .map_err(|error| format!("failed to create stdio transport: {error}"))?;
    let server = server_runtime::create_server(McpServerOptions {
        server_details: server_details(),
        transport,
        handler: MateMcpHandler {
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
        .map_err(|error| format!("mate MCP server failed: {error}"))?;
    Ok(())
}

fn server_details() -> InitializeResult {
    InitializeResult {
        server_info: Implementation {
            name: "ship".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            title: Some("Ship".to_owned()),
            description: Some("Ship mate MCP server".to_owned()),
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
        code_tool(),
        ToolDefinition {
            name: "set_plan",
            description: "Set (or change) the work plan. First call sets the plan and notifies the captain non-blocking. Subsequent calls mid-task are a blocking request: the captain must approve or reject the change before the mate can continue. Use this if you discover the scope has changed.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string", "description": "Short summary of the step (like a commit subject line)." },
                                "description": { "type": "string", "description": "Longer explanation of what the step involves." }
                            },
                            "required": ["title", "description"],
                            "additionalProperties": false
                        },
                        "minItems": 1
                    }
                },
                "required": ["steps"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "mate_ask_captain",
            description: "Ask the captain a question and wait for their response. Blocks until the captain replies via captain_steer.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" }
                },
                "required": ["question"],
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

fn mcp_tool_call_result(result: &McpToolCallResponse) -> CallToolResult {
    let structured_content = if result.diffs.is_empty() {
        None
    } else {
        let diffs: Vec<Value> = result
            .diffs
            .iter()
            .map(|d| {
                let mut obj = json!({
                    "type": "diff",
                    "path": d.path,
                    "unified_diff": d.unified_diff,
                });
                if let Some(edit_id) = &d.edit_id {
                    obj["edit_id"] = json!(edit_id);
                }
                obj
            })
            .collect();
        let mut map = serde_json::Map::new();
        map.insert("diffs".to_owned(), Value::Array(diffs));
        Some(map)
    };

    CallToolResult {
        content: vec![TextContent::from(result.text.clone()).into()],
        is_error: result.is_error.then_some(true),
        meta: None,
        structured_content,
    }
}

fn call_tool_rpc_error(error: impl std::fmt::Debug) -> CallToolError {
    CallToolError::from_message(format!("mate MCP RPC failed: {error:?}"))
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
        && !refs.is_empty()
    {
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
