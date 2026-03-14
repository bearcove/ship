use std::sync::Arc;

use super::worktree_tools::{
    ToolDefinition, commit_tool, edit_confirm_tool, edit_prepare_tool, read_file_tool,
    run_command_tool, to_sdk_tool, web_search_tool, write_file_tool,
};
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
use ship_service::CaptainMcpClient;
use ship_types::{
    AssignFileRef, CaptainAssignExtras, DirtySessionStrategy, PlanStepInput, SessionId,
};

pub struct CaptainMcpServerArgs {
    pub session_id: SessionId,
    pub server_ws_url: String,
}

#[derive(Clone)]
struct CaptainMcpHandler {
    client: CaptainMcpClient,
    tools: Arc<Vec<ToolDefinition>>,
    kagi_api_key: Option<String>,
    http_client: reqwest::Client,
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
                // r[captain.tool.assign.dirty-session-strategy]
                let dirty_session_strategy = match arguments
                    .get("dirty_session_strategy")
                    .and_then(Value::as_str)
                {
                    Some("continue_in_place") => Some(DirtySessionStrategy::ContinueInPlace),
                    Some("save_and_start_clean") => Some(DirtySessionStrategy::SaveAndStartClean),
                    Some(other) => {
                        let message = format!(
                            "invalid dirty_session_strategy: {other}. Expected one of: continue_in_place, save_and_start_clean"
                        );
                        return Ok(tool_result(&message, true));
                    }
                    None => None,
                };
                // r[captain.tool.assign.files]
                let files = arguments
                    .get("files")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| {
                                let path = v.get("path").and_then(Value::as_str)?.to_owned();
                                let start_line = v.get("start_line").and_then(Value::as_u64);
                                let end_line = v.get("end_line").and_then(Value::as_u64);
                                Some(AssignFileRef {
                                    path,
                                    start_line,
                                    end_line,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                // r[captain.tool.assign.plan]
                let plan = arguments
                    .get("plan")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| {
                                let title = v.get("title").and_then(Value::as_str)?.to_owned();
                                let description =
                                    v.get("description").and_then(Value::as_str)?.to_owned();
                                Some(PlanStepInput { title, description })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                self.client
                    .captain_assign(
                        title.to_owned(),
                        description.to_owned(),
                        keep,
                        CaptainAssignExtras {
                            files,
                            plan,
                            dirty_session_strategy,
                        },
                    )
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.steer]
            "captain_steer" => {
                let Some(message) = arguments.get("message").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: message", true));
                };
                let new_plan = match arguments.get("new_plan") {
                    Some(Value::Array(arr)) => {
                        let steps = arr
                            .iter()
                            .map(|v| {
                                let title = v.get("title").and_then(Value::as_str)?.to_owned();
                                let description =
                                    v.get("description").and_then(Value::as_str)?.to_owned();
                                Some(PlanStepInput { title, description })
                            })
                            .collect::<Option<Vec<_>>>();
                        match steps {
                            Some(s) => Some(s),
                            None => {
                                return Ok(tool_result(
                                    "new_plan: each step must have title and description",
                                    true,
                                ));
                            }
                        }
                    }
                    Some(_) => {
                        return Ok(tool_result("new_plan must be an array", true));
                    }
                    None => None,
                };
                let add_steps = match arguments.get("add_steps") {
                    Some(Value::Array(arr)) => {
                        let steps = arr
                            .iter()
                            .map(|v| {
                                let title = v.get("title").and_then(Value::as_str)?.to_owned();
                                let description =
                                    v.get("description").and_then(Value::as_str)?.to_owned();
                                Some(PlanStepInput { title, description })
                            })
                            .collect::<Option<Vec<_>>>();
                        match steps {
                            Some(s) => Some(s),
                            None => {
                                return Ok(tool_result(
                                    "add_steps: each step must have title and description",
                                    true,
                                ));
                            }
                        }
                    }
                    Some(_) => {
                        return Ok(tool_result("add_steps must be an array", true));
                    }
                    None => None,
                };
                self.client
                    .captain_steer(message.to_owned(), new_plan, add_steps)
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
            // r[captain.tool.git-status]
            "captain_git_status" => self
                .client
                .captain_git_status()
                .await
                .map_err(call_tool_rpc_error)?,
            // r[captain.tool.review-diff]
            "captain_review_diff" => self
                .client
                .captain_review_diff()
                .await
                .map_err(call_tool_rpc_error)?,
            // r[captain.tool.rebase-status]
            "captain_rebase_status" => self
                .client
                .captain_rebase_status()
                .await
                .map_err(call_tool_rpc_error)?,
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
            // r[captain.tool.read-only]
            "read_file" => {
                let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: path", true));
                };
                let offset = arguments.get("offset").and_then(Value::as_u64);
                let limit = arguments.get("limit").and_then(Value::as_u64);
                self.client
                    .captain_read_file(path.to_owned(), offset, limit)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            "run_command" => {
                let Some(command) = arguments.get("command").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: command", true));
                };
                let cwd = arguments
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                self.client
                    .captain_run_command(command.to_owned(), cwd)
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.write-file]
            "write_file" => {
                let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: path", true));
                };
                let Some(content) = arguments.get("content").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: content", true));
                };
                self.client
                    .captain_write_file(path.to_owned(), content.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.edit-prepare]
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
                    .captain_edit_prepare(
                        path.to_owned(),
                        old_string.to_owned(),
                        new_string.to_owned(),
                        replace_all,
                    )
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.edit-confirm]
            "edit_confirm" => {
                let Some(edit_id) = arguments.get("edit_id").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: edit_id", true));
                };
                self.client
                    .captain_edit_confirm(edit_id.to_owned())
                    .await
                    .map_err(call_tool_rpc_error)?
            }
            // r[captain.tool.commit]
            "commit" => {
                let Some(message) = arguments.get("message").and_then(Value::as_str) else {
                    return Ok(tool_result("missing required argument: message", true));
                };
                let step_index = arguments.get("step_index").and_then(Value::as_u64);
                self.client
                    .captain_commit(step_index, message.to_owned())
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
            // r[captain.tool.rebase-continue]
            "captain_continue_rebase" => self
                .client
                .captain_continue_rebase()
                .await
                .map_err(call_tool_rpc_error)?,
            // r[captain.tool.rebase-abort]
            "captain_abort_rebase" => self
                .client
                .captain_abort_rebase()
                .await
                .map_err(call_tool_rpc_error)?,
            other => return Err(CallToolError::unknown_tool(other.to_owned())),
        };

        Ok(tool_result(&result.text, result.is_error))
    }
}

pub async fn run_stdio_server(args: CaptainMcpServerArgs) -> Result<(), String> {
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
            description: "Assign a task to the mate. The mate will start working on it immediately. \
Set keep=true to reuse the mate's existing context; omit or set false to restart the mate with a fresh context (default). \
If the session already has leftover branch or worktree state, pass dirty_session_strategy to choose whether to continue in place or save that state and start clean. \
IMPORTANT: Always pass files and plan. Every file you read during research must be listed in files — the mate \
receives the contents directly and skips re-reading them. Your step-by-step plan must be passed via plan — the mate \
skips research and goes straight to execution. Omitting files or plan wastes the mate's time and context window.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Short title for the task (under 60 chars). Shown in the UI sidebar and headers." },
                    "description": { "type": "string", "description": "Full task description with all details the mate needs." },
                    "keep": { "type": "boolean", "description": "Reuse the mate's existing context (default false)." },
                    "dirty_session_strategy": {
                        "description": "Required when the session branch or worktree has leftover state that would otherwise be discarded before the new task starts.",
                        "oneOf": [
                            {
                                "const": "continue_in_place",
                                "description": "Continue the new task in the current worktree with the leftover state intact."
                            },
                            {
                                "const": "save_and_start_clean",
                                "description": "Save the leftover state on a timestamped branch, then reset the session branch/worktree to base before starting the new task."
                            }
                        ]
                    },
                    "files": {
                        "type": "array",
                        "description": "Files to inline into the mate's prompt. The mate receives the file contents directly — no need to re-read them.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Worktree-relative file path." },
                                "start_line": { "type": "integer", "description": "1-based first line to include (optional, defaults to start of file)." },
                                "end_line": { "type": "integer", "description": "1-based last line to include (optional, defaults to end of file)." }
                            },
                            "required": ["path"],
                            "additionalProperties": false
                        }
                    },
                    "plan": {
                        "type": "array",
                        "description": "Pre-built plan steps. If supplied, the mate skips research and planning and goes directly to execution.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string", "description": "Short summary of the step (like a commit subject line)." },
                                "description": { "type": "string", "description": "Longer explanation of what the step involves." }
                            },
                            "required": ["title", "description"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["title", "description"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_steer",
            description: "Send direction to the mate on the current task. Fire-and-forget: returns immediately. Optionally provide new_plan to replace the entire plan or add_steps to append steps — at most one may be provided.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" },
                    "new_plan": {
                        "type": "array",
                        "description": "Replace the entire plan with these steps. At most one of new_plan or add_steps may be provided.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "description": { "type": "string" }
                            },
                            "required": ["title", "description"],
                            "additionalProperties": false
                        }
                    },
                    "add_steps": {
                        "type": "array",
                        "description": "Append these steps to the existing plan. At most one of new_plan or add_steps may be provided.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "description": { "type": "string" }
                            },
                            "required": ["title", "description"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["message"],
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_accept",
            description: "Accept the mate's submitted work. Only valid after the mate calls mate_submit. Ship handles the backend-managed rebase/merge flow for this review step; do not try to do that manually with git.",
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
            name: "captain_git_status",
            description: "Inspect the current session branch state before review or accept. Reports the current branch, base branch, dirtiness, rebase state, unresolved paths, and tracked conflict markers.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_review_diff",
            description: "Rebase the session branch onto the configured base branch and return the post-rebase diff that would merge right now. If the rebase conflicts, Ship leaves the rebase in progress and reports the conflicted files instead of returning a diff.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_rebase_status",
            description: "Inspect the current rebase state, including whether a rebase is in progress and whether it is safe to continue or abort.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_notify_human",
            description: "Ask the human for guidance using the same post-rebase review diff that Ship would merge right now. Blocks until the human responds.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"],
                "additionalProperties": false,
            }),
        },
        read_file_tool(),
        run_command_tool(),
        write_file_tool(),
        edit_prepare_tool(),
        edit_confirm_tool(),
        commit_tool(),
        web_search_tool(),
        ToolDefinition {
            name: "captain_continue_rebase",
            description: "Continue a paused rebase after resolving conflicts. Ship refuses to continue while unmerged paths remain or tracked files still contain conflict markers.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolDefinition {
            name: "captain_abort_rebase",
            description: "Abort the in-progress rebase and return the session worktree to its pre-rebase state.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
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
