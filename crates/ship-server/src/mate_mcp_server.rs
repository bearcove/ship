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
use ship_service::MateMcpClient;
use ship_types::{McpToolCallResponse, SessionId};

pub struct MateMcpServerArgs {
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
            // r[mate.tool.run-command]
            "run_command" => {
                let Some(command) = arguments.get("command").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: command", true));
                };
                let cwd = match arguments.get("cwd") {
                    Some(value) => Some(
                        value
                            .as_str()
                            .ok_or_else(|| call_tool_rpc_error("cwd must be a string"))?,
                    ),
                    None => None,
                };
                self.client
                    .run_command(command.to_owned(), cwd.map(ToOwned::to_owned))
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.read-file]
            "read_file" => {
                let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: path", true));
                };
                let offset = match arguments.get("offset") {
                    Some(value) => Some(
                        value
                            .as_u64()
                            .ok_or_else(|| call_tool_rpc_error("offset must be an integer"))?,
                    ),
                    None => None,
                };
                let limit = match arguments.get("limit") {
                    Some(value) => Some(
                        value
                            .as_u64()
                            .ok_or_else(|| call_tool_rpc_error("limit must be an integer"))?,
                    ),
                    None => None,
                };
                self.client
                    .read_file(path.to_owned(), offset, limit)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.write-file]
            "write_file" => {
                let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: path", true));
                };
                let Some(content) = arguments.get("content").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: content", true));
                };
                self.client
                    .write_file(path.to_owned(), content.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.edit-prepare]
            "edit_prepare" => {
                let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: path", true));
                };
                let Some(old_string) = arguments.get("old_string").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: old_string", true));
                };
                let Some(new_string) = arguments.get("new_string").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: new_string", true));
                };
                let replace_all = match arguments.get("replace_all") {
                    Some(value) => Some(
                        value
                            .as_bool()
                            .ok_or_else(|| call_tool_rpc_error("replace_all must be a boolean"))?,
                    ),
                    None => None,
                };
                self.client
                    .edit_prepare(
                        path.to_owned(),
                        old_string.to_owned(),
                        new_string.to_owned(),
                        replace_all,
                    )
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.edit-confirm]
            "edit_confirm" => {
                let Some(edit_id) = arguments.get("edit_id").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: edit_id", true));
                };
                self.client
                    .edit_confirm(edit_id.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.search-files]
            "search_files" => {
                let Some(args) = arguments.get("args").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: args", true));
                };
                self.client
                    .search_files(args.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.list-files]
            "list_files" => {
                let Some(args) = arguments.get("args").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: args", true));
                };
                self.client
                    .list_files(args.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.send-update]
            "mate_send_update" => {
                let Some(message) = arguments.get("message").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: message", true));
                };
                self.client
                    .mate_send_update(message.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.plan-create]
            "set_plan" => {
                let Some(steps) = arguments.get("steps").and_then(Value::as_array) else {
                    return Ok(tool_result("missing required argument: steps", true));
                };
                let steps = steps
                    .iter()
                    .map(|value| value.as_str().map(ToOwned::to_owned))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| call_tool_rpc_error("steps must be strings"))?;
                self.client
                    .set_plan(steps)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.plan-step-complete]
            "plan_step_complete" => {
                let Some(step_index) = arguments.get("step_index").and_then(Value::as_u64) else {
                    return Ok(tool_result("missing required argument: step_index", true));
                };
                let Some(summary) = arguments.get("summary").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: summary", true));
                };
                self.client
                    .plan_step_complete(step_index, summary.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.cargo-check]
            "cargo_check" => {
                let args = arguments
                    .get("args")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.client
                    .cargo_check(args)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.cargo-clippy]
            "cargo_clippy" => {
                let args = arguments
                    .get("args")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.client
                    .cargo_clippy(args)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.cargo-test]
            "cargo_test" => {
                let args = arguments
                    .get("args")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.client
                    .cargo_test(args)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[mate.tool.pnpm-install]
            "pnpm_install" => {
                let args = arguments
                    .get("args")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.client
                    .pnpm_install(args)
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
            // r[mate.tool.submit]
            "mate_submit" => {
                let Some(summary) = arguments.get("summary").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: summary", true));
                };
                self.client
                    .mate_submit(summary.to_owned())
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
        ToolDefinition {
            name: "run_command",
            description: "Run a shell command in the current task worktree via sh -c. Use this as a general-purpose escape hatch when no structured tool fits. Some risky commands require captain approval first. Optional cwd is relative to the worktree root.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "cwd": { "type": "string" }
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "read_file",
            description: "Read a text file from the current task worktree with line numbers. Supports optional 1-based offset and line limit.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "offset": { "type": "integer", "minimum": 1 },
                    "limit": { "type": "integer", "minimum": 1 }
                },
                "required": ["path"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "write_file",
            description: "Write a file in the current task worktree. Rust files are syntax-checked with rustfmt and auto-formatted before the write is committed.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "edit_prepare",
            description: "Prepare a search-and-replace edit. Returns a diff preview without modifying the file. The response includes an edit_id in the structured content (diff.edit_id) and in the text. You MUST call edit_confirm with that edit_id to apply the edit.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_string": { "type": "string" },
                    "new_string": { "type": "string" },
                    "replace_all": { "type": "boolean" }
                },
                "required": ["path", "old_string", "new_string"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "edit_confirm",
            description: "Apply a previously prepared edit. Pass the edit_id exactly as returned by edit_prepare (from the structured content diff.edit_id field, or from the text response). Runs syntax validation for Rust files. If validation fails, the file is not modified and the error is returned.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "edit_id": { "type": "string" }
                },
                "required": ["edit_id"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "search_files",
            description: "Search file contents using ripgrep (rg). Args are passed directly to rg.\nRuns in the worktree root. Output is line-numbered matches.\n\nKey flags:\n  rg PATTERN [PATH...]          Search for pattern\n  -t, --type TYPE               Filter by file type (rust, ts, py, json, ...)\n  -g, --glob GLOB               Include/exclude files by glob (-g '!vendor/')\n  -i, --ignore-case             Case-insensitive search\n  -w, --word-regexp             Match whole words only\n  -l, --files-with-matches      Only print file names\n  -c, --count                   Only print match counts per file\n  -C NUM, --context NUM         Show NUM lines of context\n  -n, --line-number             Show line numbers (default)\n  -F, --fixed-strings           Treat pattern as literal string\n  --multiline                   Match across line boundaries\n\nExample: \"fn handle_.*event\" -t rust src/",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string" }
                },
                "required": ["args"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "list_files",
            description: "List files using fd. Args are passed directly to fd.\nRuns in the worktree root.\n\nKey flags:\n  fd [PATTERN] [PATH...]        Search for files matching pattern\n  -e, --extension EXT           Filter by extension\n  -t, --type TYPE               f=file, d=directory, l=symlink\n  -d, --max-depth DEPTH         Limit directory traversal depth\n  -H, --hidden                  Include hidden files\n  -g, --glob PATTERN            Glob-based search instead of regex\n  -E, --exclude PATTERN         Exclude entries matching pattern\n\nExample: -e rs src/",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string" }
                },
                "required": ["args"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "mate_send_update",
            description: "Send a progress update to the captain. Returns immediately without waiting for a response.",
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
            name: "set_plan",
            description: "Set (or change) the work plan. First call sets the plan and notifies the captain non-blocking. Subsequent calls mid-task are a blocking request: the captain must approve or reject the change before the mate can continue. Use this if you discover the scope has changed.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 1
                    }
                },
                "required": ["steps"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "plan_step_complete",
            description: "Mark the current plan step complete and commit its changes. Call this IMMEDIATELY after finishing each step — before starting the next one. All file changes for this step must already be written. This commits them as one focused commit. If called after starting the next step's changes, those changes will be bundled into the wrong commit.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "step_index": { "type": "integer", "minimum": 0 },
                    "summary": { "type": "string" }
                },
                "required": ["step_index", "summary"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "cargo_check",
            description: "Run `cargo check` in the worktree. Network-enabled sandbox; fetches missing dependencies. Takes an optional args string.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string" }
                },
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "cargo_clippy",
            description: "Run `cargo clippy` in the worktree. Network-enabled sandbox. Takes an optional args string.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string" }
                },
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "cargo_test",
            description: "Run `cargo nextest run` in the worktree. Network-enabled sandbox. Takes an optional args string.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string" }
                },
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "pnpm_install",
            description: "Run `pnpm install` in the worktree. Network-enabled sandbox. Takes an optional args string.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string" }
                },
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
        ToolDefinition {
            name: "mate_submit",
            description: "Submit completed work for captain review. Blocks until the captain accepts, steers, or cancels.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "web_search",
            description: "Search the web using Kagi FastGPT. Returns an AI-synthesized answer and a list of references.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"],
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
                    "new_text": d.new_text,
                });
                if let Some(old) = &d.old_text {
                    obj["old_text"] = json!(old);
                }
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
    {
        if !refs.is_empty() {
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
    }
    tool_result(&text, false)
}
