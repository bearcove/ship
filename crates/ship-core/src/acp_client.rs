use similar::TextDiff;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use agent_client_protocol::{
    Client, ContentBlock, CreateTerminalRequest, CreateTerminalResponse, Error,
    KillTerminalCommandRequest, KillTerminalCommandResponse, PermissionOptionKind,
    ReadTextFileRequest, ReadTextFileResponse, ReleaseTerminalRequest, ReleaseTerminalResponse,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    Result as AcpResult, SelectedPermissionOutcome, SessionNotification, SessionUpdate,
    TerminalExitStatus, TerminalOutputRequest, TerminalOutputResponse, ToolCallContent,
    ToolCallStatus, ToolKind, WaitForTerminalExitRequest, WaitForTerminalExitResponse,
    WriteTextFileRequest, WriteTextFileResponse,
};
use ship_types::{
    AgentState, BlockId, BlockPatch, ContentBlock as ShipContentBlock, JsonEntry,
    JsonValue as ShipJsonValue, PermissionOption, PermissionOptionKind as ShipPermissionOptionKind,
    PermissionRequest, PlanStep, PlanStepStatus, Role, SessionEvent,
    TerminalExit as ShipTerminalExit, TerminalSnapshot as ShipTerminalSnapshot, TextSource,
    ToolCallContent as ShipToolCallContent, ToolCallError as ShipToolCallError,
    ToolCallKind as ShipToolCallKind, ToolCallLocation as ShipToolCallLocation,
    ToolCallStatus as ShipToolCallStatus, ToolTarget as ShipToolTarget,
};
use tokio::process::Child;
use tokio::sync::{mpsc, oneshot, watch};

pub struct ShipAcpClient {
    role: Role,
    worktree_path: PathBuf,
    notifications_tx: mpsc::UnboundedSender<SessionEvent>,
    active_text_block: Rc<RefCell<Option<(BlockId, TextSource)>>>,
    tool_call_blocks: Rc<RefCell<HashMap<String, BlockId>>>,
    tool_calls: Rc<RefCell<HashMap<String, agent_client_protocol::ToolCall>>>,
    pending_permissions: Rc<RefCell<HashMap<String, oneshot::Sender<String>>>>,
    terminals: Rc<RefCell<HashMap<String, TerminalState>>>,
}

struct TerminalState {
    child: Rc<RefCell<Child>>,
    output: Rc<RefCell<String>>,
    exit_status: Rc<RefCell<Option<TerminalExitStatus>>>,
    exit_status_rx: watch::Receiver<Option<TerminalExitStatus>>,
}

impl ShipAcpClient {
    pub fn new(
        role: Role,
        worktree_path: PathBuf,
        notifications_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Self {
        Self {
            role,
            worktree_path,
            notifications_tx,
            active_text_block: Rc::new(RefCell::new(None)),
            tool_call_blocks: Rc::new(RefCell::new(HashMap::new())),
            tool_calls: Rc::new(RefCell::new(HashMap::new())),
            pending_permissions: Rc::new(RefCell::new(HashMap::new())),
            terminals: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn begin_prompt_turn(&self) {
        self.reset_text_block();
    }

    pub fn resolve_permission(&self, permission_id: &str, option_id: &str) -> Result<(), String> {
        let sender = self
            .pending_permissions
            .borrow_mut()
            .remove(permission_id)
            .ok_or_else(|| format!("pending permission not found: {permission_id}"))?;
        sender
            .send(option_id.to_owned())
            .map_err(|_| format!("permission receiver dropped: {permission_id}"))
    }

    fn send_event(&self, event: SessionEvent) {
        let _ = self.notifications_tx.send(event);
    }

    fn reset_text_block(&self) {
        *self.active_text_block.borrow_mut() = None;
    }

    // r[captain.capabilities]
    // r[mate.capabilities]
    // Agents run in sandboxed worktrees so most built-in tools are safe.
    // Edit/Delete of code files (.rs, .ts, .tsx, .js, .jsx) are blocked —
    // they bypass the MCP layer's rustfmt validation and structural integrity
    // checks. Non-code files (e.g. .md plans) are allowed through.
    fn blocked_permission_option_id(&self, request: &RequestPermissionRequest) -> Option<String> {
        let is_write_tool = request
            .tool_call
            .fields
            .kind
            .as_ref()
            .is_some_and(|kind| matches!(kind, ToolKind::Edit | ToolKind::Delete));
        if !is_write_tool {
            return None;
        }

        let targets_code_file = request
            .tool_call
            .fields
            .locations
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .any(|loc| {
                let ext = loc.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx")
            });
        if !targets_code_file {
            return None;
        }

        request.options.iter().find_map(|option| {
            matches!(
                option.kind,
                PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways
            )
            .then_some(option.option_id.0.to_string())
        })
    }

    fn append_text_chunk(&self, text: String, source: TextSource) -> Vec<SessionEvent> {
        let existing_block = self.active_text_block.borrow().clone();
        match existing_block {
            Some((block_id, existing_source)) if existing_source == source => {
                vec![SessionEvent::BlockPatch {
                    block_id,
                    role: self.role,
                    patch: BlockPatch::TextAppend { text },
                }]
            }
            _ => {
                let block_id = BlockId::new();
                *self.active_text_block.borrow_mut() = Some((block_id.clone(), source));
                vec![SessionEvent::BlockAppend {
                    block_id,
                    role: self.role,
                    block: ShipContentBlock::Text { text, source },
                }]
            }
        }
    }

    // r[acp.content-blocks]
    // r[acp.terminals]
    fn upsert_tool_call(&self, tool_call: agent_client_protocol::ToolCall) -> Vec<SessionEvent> {
        let tool_call_id = tool_call.tool_call_id.0.to_string();
        self.tool_calls
            .borrow_mut()
            .insert(tool_call_id.clone(), tool_call.clone());
        let ship_block = self.map_tool_call_block(&tool_call);

        let existing_block = self.tool_call_blocks.borrow().get(&tool_call_id).cloned();
        match existing_block {
            Some(block_id) => vec![SessionEvent::BlockPatch {
                block_id,
                role: self.role,
                patch: map_tool_call_patch(&ship_block),
            }],
            None => {
                let block_id = BlockId::new();
                self.tool_call_blocks
                    .borrow_mut()
                    .insert(tool_call_id, block_id.clone());
                vec![SessionEvent::BlockAppend {
                    block_id,
                    role: self.role,
                    block: ship_block,
                }]
            }
        }
    }

    fn apply_tool_call_update(
        &self,
        update: agent_client_protocol::ToolCallUpdate,
    ) -> Vec<SessionEvent> {
        let tool_call_id = update.tool_call_id.0.to_string();
        let mut tool_calls = self.tool_calls.borrow_mut();
        let tool_call = tool_calls.entry(tool_call_id).or_insert_with(|| {
            let title = update
                .fields
                .title
                .clone()
                .unwrap_or_else(|| "ACP tool call".to_owned());
            agent_client_protocol::ToolCall::new(update.tool_call_id.clone(), title)
        });
        tool_call.update(update.fields);
        let ship_block = self.map_tool_call_block(tool_call);
        drop(tool_calls);

        let tool_call_id = ship_block_tool_call_id(&ship_block).unwrap_or_default();
        let existing_block = self.tool_call_blocks.borrow().get(&tool_call_id).cloned();
        match existing_block {
            Some(block_id) => vec![SessionEvent::BlockPatch {
                block_id,
                role: self.role,
                patch: map_tool_call_patch(&ship_block),
            }],
            None => {
                let block_id = BlockId::new();
                self.tool_call_blocks
                    .borrow_mut()
                    .insert(tool_call_id, block_id.clone());
                vec![SessionEvent::BlockAppend {
                    block_id,
                    role: self.role,
                    block: ship_block,
                }]
            }
        }
    }

    fn map_tool_call_block(&self, tool_call: &agent_client_protocol::ToolCall) -> ShipContentBlock {
        let tool_call_id = tool_call.tool_call_id.0.to_string();
        let raw_input = tool_call.raw_input.as_ref().map(map_json_value);
        let raw_output = tool_call.raw_output.as_ref().map(map_json_value);

        let mut content =
            map_tool_call_contents(&self.worktree_path, &self.terminals, &tool_call.content);

        // Extract diffs from raw_output (injected by mate MCP server via structured_content)
        if let Some(raw) = &tool_call.raw_output {
            if let Some(diffs) = raw.get("diffs").and_then(|v| v.as_array()) {
                for diff in diffs {
                    if diff.get("type").and_then(|t| t.as_str()) != Some("diff") {
                        continue;
                    }
                    let Some(path) = diff.get("path").and_then(|p| p.as_str()) else {
                        continue;
                    };
                    let unified_diff = diff
                        .get("unified_diff")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_owned();
                    content.push(ShipToolCallContent::Diff {
                        path: path.to_owned(),
                        display_path: display_path_for_string(&self.worktree_path, path),
                        unified_diff,
                    });
                }
            }
        }

        ShipContentBlock::ToolCall {
            tool_call_id: Some(tool_call_id),
            tool_name: tool_call.title.clone(),
            arguments: tool_call
                .raw_input
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            kind: Some(map_tool_kind(tool_call.kind)),
            target: map_tool_target(
                &self.worktree_path,
                tool_call.kind,
                tool_call.raw_input.as_ref(),
                &tool_call.locations,
            ),
            raw_input,
            raw_output,
            locations: map_tool_call_locations(&self.worktree_path, &tool_call.locations),
            status: map_tool_status(tool_call.status),
            content,
            error: map_tool_error(tool_call),
        }
    }

    // r[acp.notifications]
    fn map_session_update(&self, update: SessionUpdate) -> Vec<SessionEvent> {
        match update {
            SessionUpdate::UserMessageChunk(chunk) => {
                self.append_text_chunk(format_content_block(chunk.content), TextSource::Human)
            }
            SessionUpdate::AgentMessageChunk(chunk) => self.append_text_chunk(
                format_content_block(chunk.content),
                TextSource::AgentMessage,
            ),
            SessionUpdate::AgentThoughtChunk(chunk) => self.append_text_chunk(
                format_content_block(chunk.content),
                TextSource::AgentThought,
            ),
            SessionUpdate::ToolCall(tool_call) => {
                self.reset_text_block();
                self.upsert_tool_call(tool_call)
            }
            SessionUpdate::ToolCallUpdate(update) => {
                self.reset_text_block();
                self.apply_tool_call_update(update)
            }
            // r[acp.plans]
            SessionUpdate::Plan(plan) => {
                self.reset_text_block();
                vec![SessionEvent::AgentStateChanged {
                    role: self.role,
                    state: AgentState::Working {
                        plan: Some(
                            plan.entries
                                .into_iter()
                                .map(|entry| PlanStep {
                                    title: String::new(),
                                    description: entry.content,
                                    status: map_plan_status(entry.status),
                                })
                                .collect(),
                        ),
                        activity: Some("ACP plan update".to_owned()),
                    },
                }]
            }
            SessionUpdate::AvailableCommandsUpdate(_)
            | SessionUpdate::CurrentModeUpdate(_)
            | SessionUpdate::ConfigOptionUpdate(_) => {
                self.reset_text_block();
                Vec::new()
            }
            // r[event.context-updated]
            SessionUpdate::UsageUpdate(update) => {
                let Some(remaining_percent) = remaining_context_percent(update.used, update.size)
                else {
                    return Vec::new();
                };
                vec![SessionEvent::ContextUpdated {
                    role: self.role,
                    remaining_percent,
                }]
            }
            _ => {
                self.reset_text_block();
                Vec::new()
            }
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Client for ShipAcpClient {
    // r[acp.client.permission]
    // r[acp.permissions]
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> AcpResult<RequestPermissionResponse> {
        self.reset_text_block();

        // Auto-reject built-in tool permissions silently, before emitting any UI events.
        if let Some(option_id) = self.blocked_permission_option_id(&args) {
            tracing::warn!(role = ?self.role, "auto-rejected ACP built-in tool permission request");
            return Ok(RequestPermissionResponse::new(
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id)),
            ));
        }

        // Auto-approve non-built-in tool permissions (e.g. ExitPlanMode).
        // There's no human to click approve, so select the first allow option.
        if let Some(allow_id) = args.options.iter().find_map(|option| {
            matches!(
                option.kind,
                PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
            )
            .then_some(option.option_id.0.to_string())
        }) {
            tracing::info!(
                role = ?self.role,
                title = ?args.tool_call.fields.title,
                "auto-approved non-built-in tool permission request"
            );
            return Ok(RequestPermissionResponse::new(
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(allow_id)),
            ));
        }

        let permission_id = args.tool_call.tool_call_id.0.to_string();
        let tool_call_id = args.tool_call.tool_call_id.0.to_string();
        let tool_name = args
            .tool_call
            .fields
            .title
            .clone()
            .unwrap_or_else(|| "ACP permission request".to_owned());
        let arguments = args
            .tool_call
            .fields
            .raw_input
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        let description = tool_name.clone();
        let kind = args.tool_call.fields.kind.map(map_tool_kind);
        let target = map_tool_target(
            &self.worktree_path,
            args.tool_call.fields.kind.unwrap_or(ToolKind::Other),
            args.tool_call.fields.raw_input.as_ref(),
            &[],
        );
        let raw_input = args.tool_call.fields.raw_input.as_ref().map(map_json_value);
        let options = Some(map_permission_options(&args.options));
        let block_id = BlockId::new();

        self.send_event(SessionEvent::BlockAppend {
            block_id: block_id.clone(),
            role: self.role,
            block: ShipContentBlock::Permission {
                permission_id: Some(permission_id.clone()),
                tool_call_id: Some(tool_call_id.clone()),
                tool_name: tool_name.clone(),
                description: description.clone(),
                arguments: arguments.clone(),
                kind,
                target: target.clone(),
                raw_input: raw_input.clone(),
                options: options.clone(),
                resolution: None,
            },
        });
        self.send_event(SessionEvent::AgentStateChanged {
            role: self.role,
            state: AgentState::AwaitingPermission {
                request: PermissionRequest {
                    permission_id: permission_id.clone(),
                    tool_call_id: Some(tool_call_id),
                    tool_name,
                    arguments,
                    description,
                    kind,
                    target,
                    raw_input,
                    options,
                },
            },
        });

        let (resolution_tx, resolution_rx) = oneshot::channel();
        self.pending_permissions
            .borrow_mut()
            .insert(permission_id.clone(), resolution_tx);

        let selected_option_id = resolution_rx.await.ok();

        self.send_event(SessionEvent::AgentStateChanged {
            role: self.role,
            state: AgentState::Working {
                plan: None,
                activity: Some("Permission resolved".to_owned()),
            },
        });

        let outcome = match selected_option_id {
            Some(option_id) => {
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id))
            }
            None => RequestPermissionOutcome::Cancelled,
        };

        Ok(RequestPermissionResponse::new(outcome))
    }

    // r[acp.client.session-notification]
    async fn session_notification(&self, args: SessionNotification) -> AcpResult<()> {
        for event in self.map_session_update(args.update) {
            self.send_event(event);
        }
        Ok(())
    }

    // r[acp.client.fs-write]
    // r[captain.capabilities]
    // r[mate.capabilities]
    // Code files (.rs, .ts, .tsx) must go through the MCP write_file tool
    // (which validates Rust files with rustfmt). Other files (e.g. .md plans)
    // are allowed through the built-in write.
    async fn write_text_file(
        &self,
        args: WriteTextFileRequest,
    ) -> AcpResult<WriteTextFileResponse> {
        let ext = args.path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx") {
            tracing::warn!(role = ?self.role, path = %args.path.display(), "rejected ACP built-in write_text_file for code file");
            return Err(Error::invalid_params().data(
                "Built-in file writing is disabled for code files. Use the write_file MCP tool instead (it validates Rust files with rustfmt).",
            ));
        }
        tracing::info!(role = ?self.role, path = %args.path.display(), "allowing ACP built-in write_text_file");
        Ok(WriteTextFileResponse::new())
    }

    // read_text_file and create_terminal capabilities are set to false,
    // so the agent uses its own sandboxed built-ins. These should never
    // be called, but we keep them as a safety net.
    async fn read_text_file(&self, _args: ReadTextFileRequest) -> AcpResult<ReadTextFileResponse> {
        tracing::warn!(role = ?self.role, "unexpected read_text_file call (capability is false)");
        Err(Error::invalid_params().data("Unexpected: read_text_file should not be routed here."))
    }

    async fn create_terminal(
        &self,
        _args: CreateTerminalRequest,
    ) -> AcpResult<CreateTerminalResponse> {
        tracing::warn!(role = ?self.role, "unexpected create_terminal call (capability is false)");
        Err(Error::invalid_params().data("Unexpected: create_terminal should not be routed here."))
    }

    // r[acp.client.terminal-output]
    async fn terminal_output(
        &self,
        args: TerminalOutputRequest,
    ) -> AcpResult<TerminalOutputResponse> {
        let terminal_id = args.terminal_id.0.to_string();
        let terminals = self.terminals.borrow();
        let terminal = terminals
            .get(&terminal_id)
            .ok_or_else(Error::invalid_params)?;

        let mut response = TerminalOutputResponse::new(terminal.output.borrow().clone(), false);
        if let Some(exit_status) = terminal.exit_status.borrow().clone() {
            response = response.exit_status(exit_status);
        }
        Ok(response)
    }

    // r[acp.client.terminal-wait]
    async fn wait_for_terminal_exit(
        &self,
        args: WaitForTerminalExitRequest,
    ) -> AcpResult<WaitForTerminalExitResponse> {
        let terminal_id = args.terminal_id.0.to_string();
        let mut exit_status_rx = {
            let terminals = self.terminals.borrow();
            let terminal = terminals
                .get(&terminal_id)
                .ok_or_else(Error::invalid_params)?;
            if let Some(exit_status) = terminal.exit_status.borrow().clone() {
                return Ok(WaitForTerminalExitResponse::new(exit_status));
            }
            terminal.exit_status_rx.clone()
        };

        loop {
            if let Some(exit_status) = exit_status_rx.borrow().clone() {
                return Ok(WaitForTerminalExitResponse::new(exit_status));
            }
            if exit_status_rx.changed().await.is_err() {
                return Err(Error::internal_error());
            }
        }
    }

    // r[acp.client.terminal-kill]
    async fn kill_terminal_command(
        &self,
        args: KillTerminalCommandRequest,
    ) -> AcpResult<KillTerminalCommandResponse> {
        let terminal_id = args.terminal_id.0.to_string();
        let terminals = self.terminals.borrow();
        let terminal = terminals
            .get(&terminal_id)
            .ok_or_else(Error::invalid_params)?;
        kill_if_running(&terminal.child).map_err(|_| Error::internal_error())?;
        Ok(KillTerminalCommandResponse::new())
    }

    // r[acp.client.terminal-release]
    async fn release_terminal(
        &self,
        args: ReleaseTerminalRequest,
    ) -> AcpResult<ReleaseTerminalResponse> {
        let terminal_id = args.terminal_id.0.to_string();
        let terminal = self
            .terminals
            .borrow_mut()
            .remove(&terminal_id)
            .ok_or_else(Error::invalid_params)?;
        let _ = kill_if_running(&terminal.child);
        Ok(ReleaseTerminalResponse::new())
    }
}

fn kill_if_running(child: &Rc<RefCell<Child>>) -> std::io::Result<()> {
    let mut child = child.borrow_mut();
    if child.try_wait()?.is_none() {
        child.start_kill()?;
    }
    Ok(())
}

fn format_content_block(block: ContentBlock) -> String {
    match block {
        ContentBlock::Text(text) => text.text,
        ContentBlock::Image(image) => image
            .uri
            .unwrap_or_else(|| format!("[image {}]", image.mime_type)),
        ContentBlock::Audio(audio) => format!("[audio {}]", audio.mime_type),
        ContentBlock::ResourceLink(link) => link.title.unwrap_or(link.name),
        ContentBlock::Resource(resource) => match resource.resource {
            agent_client_protocol::EmbeddedResourceResource::TextResourceContents(text) => {
                text.text
            }
            agent_client_protocol::EmbeddedResourceResource::BlobResourceContents(blob) => {
                format!("[resource {}]", blob.uri)
            }
            _ => "[resource]".to_owned(),
        },
        _ => "[unsupported content]".to_owned(),
    }
}

fn ship_block_tool_call_id(block: &ShipContentBlock) -> Option<String> {
    match block {
        ShipContentBlock::ToolCall { tool_call_id, .. } => tool_call_id.clone(),
        _ => None,
    }
}

fn map_tool_call_patch(block: &ShipContentBlock) -> BlockPatch {
    match block {
        ShipContentBlock::ToolCall {
            tool_name,
            kind,
            target,
            raw_input,
            raw_output,
            status,
            locations,
            content,
            error,
            ..
        } => BlockPatch::ToolCallUpdate {
            tool_name: Some(tool_name.clone()),
            kind: *kind,
            target: target.clone(),
            raw_input: raw_input.clone(),
            raw_output: raw_output.clone(),
            status: *status,
            locations: Some(locations.clone()),
            content: Some(content.clone()),
            error: error.clone(),
        },
        _ => unreachable!("tool call patch must come from a tool call block"),
    }
}

fn map_tool_call_locations(
    worktree_path: &Path,
    locations: &[agent_client_protocol::ToolCallLocation],
) -> Vec<ShipToolCallLocation> {
    locations
        .iter()
        .map(|location| ShipToolCallLocation {
            path: location.path.display().to_string(),
            display_path: display_path_for_path(worktree_path, &location.path),
            line: location.line,
        })
        .collect()
}

fn map_tool_call_contents(
    worktree_path: &Path,
    terminals: &Rc<RefCell<HashMap<String, TerminalState>>>,
    blocks: &[ToolCallContent],
) -> Vec<ShipToolCallContent> {
    blocks
        .iter()
        .map(|content| map_tool_call_content(worktree_path, terminals, content))
        .collect()
}

fn map_tool_call_content(
    worktree_path: &Path,
    terminals: &Rc<RefCell<HashMap<String, TerminalState>>>,
    content: &ToolCallContent,
) -> ShipToolCallContent {
    match content {
        ToolCallContent::Content(content) => match &content.content {
            ContentBlock::Text(text) => ShipToolCallContent::Text {
                text: text.text.clone(),
            },
            other => ShipToolCallContent::Raw {
                data: map_json_value(
                    &serde_json::to_value(other).unwrap_or(serde_json::Value::Null),
                ),
            },
        },
        ToolCallContent::Diff(diff) => {
            let path_str = diff.path.display().to_string();
            let old = diff.old_text.as_deref().unwrap_or("");
            let unified_diff = TextDiff::from_lines(old, &diff.new_text)
                .unified_diff()
                .context_radius(3)
                .header(&format!("a/{path_str}"), &format!("b/{path_str}"))
                .to_string();
            ShipToolCallContent::Diff {
                path: path_str,
                display_path: display_path_for_path(worktree_path, &diff.path),
                unified_diff,
            }
        }
        ToolCallContent::Terminal(terminal) => ShipToolCallContent::Terminal {
            terminal_id: terminal.terminal_id.0.to_string(),
            snapshot: map_terminal_snapshot(terminals, &terminal.terminal_id.0),
        },
        _ => ShipToolCallContent::Raw {
            data: map_json_value(&serde_json::to_value(content).unwrap_or(serde_json::Value::Null)),
        },
    }
}

fn map_tool_status(status: ToolCallStatus) -> ShipToolCallStatus {
    match status {
        ToolCallStatus::Pending | ToolCallStatus::InProgress => ShipToolCallStatus::Running,
        ToolCallStatus::Completed => ShipToolCallStatus::Success,
        ToolCallStatus::Failed => ShipToolCallStatus::Failure,
        _ => ShipToolCallStatus::Running,
    }
}

fn map_tool_kind(kind: ToolKind) -> ShipToolCallKind {
    match kind {
        ToolKind::Read => ShipToolCallKind::Read,
        ToolKind::Edit => ShipToolCallKind::Edit,
        ToolKind::Delete => ShipToolCallKind::Delete,
        ToolKind::Move => ShipToolCallKind::Move,
        ToolKind::Search => ShipToolCallKind::Search,
        ToolKind::Execute => ShipToolCallKind::Execute,
        ToolKind::Think => ShipToolCallKind::Think,
        ToolKind::Fetch => ShipToolCallKind::Fetch,
        ToolKind::SwitchMode => ShipToolCallKind::SwitchMode,
        _ => ShipToolCallKind::Other,
    }
}

fn map_tool_target(
    worktree_path: &Path,
    kind: ToolKind,
    raw_input: Option<&serde_json::Value>,
    locations: &[agent_client_protocol::ToolCallLocation],
) -> Option<ShipToolTarget> {
    match kind {
        ToolKind::Read | ToolKind::Edit | ToolKind::Delete => {
            map_file_target(worktree_path, raw_input, locations)
        }
        ToolKind::Move => map_move_target(worktree_path, raw_input),
        ToolKind::Search => map_search_target(worktree_path, raw_input),
        ToolKind::Execute => map_command_target(worktree_path, raw_input),
        _ => map_command_target(worktree_path, raw_input)
            .or_else(|| map_search_target(worktree_path, raw_input))
            .or_else(|| map_file_target(worktree_path, raw_input, locations)),
    }
}

fn map_file_target(
    worktree_path: &Path,
    raw_input: Option<&serde_json::Value>,
    locations: &[agent_client_protocol::ToolCallLocation],
) -> Option<ShipToolTarget> {
    if let Some(path) = raw_input
        .and_then(|value| extract_string(value, &["path", "file_path", "file", "filepath"]))
    {
        let line = raw_input.and_then(|value| extract_u32(value, &["line", "start_line"]));
        return Some(ShipToolTarget::File {
            display_path: display_path_for_string(worktree_path, &path),
            path,
            line,
        });
    }

    locations.first().map(|location| ShipToolTarget::File {
        path: location.path.display().to_string(),
        display_path: display_path_for_path(worktree_path, &location.path),
        line: location.line,
    })
}

fn map_move_target(
    worktree_path: &Path,
    raw_input: Option<&serde_json::Value>,
) -> Option<ShipToolTarget> {
    let raw_input = raw_input?;
    let source_path = extract_string(raw_input, &["source", "src", "from", "old_path"])?;
    let destination_path = extract_string(
        raw_input,
        &["destination", "dest", "to", "new_path", "path"],
    )?;
    Some(ShipToolTarget::Move {
        source_display_path: display_path_for_string(worktree_path, &source_path),
        source_path,
        destination_display_path: display_path_for_string(worktree_path, &destination_path),
        destination_path,
    })
}

fn map_search_target(
    worktree_path: &Path,
    raw_input: Option<&serde_json::Value>,
) -> Option<ShipToolTarget> {
    let raw_input = raw_input?;
    let query = extract_string(raw_input, &["pattern", "query", "search", "regex", "term"]);
    let glob = extract_string(raw_input, &["glob", "include"]);
    let path = extract_string(raw_input, &["path", "cwd", "directory", "root"]);

    if query.is_none() && glob.is_none() && path.is_none() {
        return None;
    }

    Some(ShipToolTarget::Search {
        display_path: path
            .as_ref()
            .and_then(|value| display_path_for_string(worktree_path, value)),
        path,
        query,
        glob,
    })
}

fn map_command_target(
    worktree_path: &Path,
    raw_input: Option<&serde_json::Value>,
) -> Option<ShipToolTarget> {
    let raw_input = raw_input?;
    let command = build_command(raw_input)?;
    let cwd = extract_string(raw_input, &["cwd", "workdir", "directory"]);

    Some(ShipToolTarget::Command {
        display_cwd: cwd
            .as_ref()
            .and_then(|value| display_path_for_string(worktree_path, value)),
        cwd,
        command,
    })
}

fn build_command(raw_input: &serde_json::Value) -> Option<String> {
    if let Some(command) = extract_string(raw_input, &["command", "cmd"]) {
        let args = extract_string_array(raw_input, &["args", "argv"]).unwrap_or_default();
        if args.is_empty() {
            return Some(command);
        }
        let mut parts = vec![command];
        parts.extend(args);
        return Some(parts.join(" "));
    }

    extract_string_array(raw_input, &["argv", "command"]).and_then(|parts| {
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    })
}

fn map_tool_error(tool_call: &agent_client_protocol::ToolCall) -> Option<ShipToolCallError> {
    if !matches!(tool_call.status, ToolCallStatus::Failed) {
        return None;
    }

    let details = tool_call.raw_output.as_ref().map(map_json_value);
    let message = tool_call
        .raw_output
        .as_ref()
        .and_then(extract_error_message)
        .unwrap_or_else(|| format!("{} failed", tool_call.title));

    Some(ShipToolCallError { message, details })
}

fn extract_error_message(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Array(items) => items.iter().find_map(extract_error_message),
        serde_json::Value::Object(map) => {
            for key in ["error", "message", "stderr", "text", "detail", "content"] {
                if let Some(value) = map.get(key)
                    && let Some(message) = extract_error_message(value)
                {
                    return Some(message);
                }
            }
            None
        }
        _ => None,
    }
}

fn map_terminal_snapshot(
    terminals: &Rc<RefCell<HashMap<String, TerminalState>>>,
    terminal_id: &str,
) -> Option<ShipTerminalSnapshot> {
    let terminals = terminals.borrow();
    let terminal = terminals.get(terminal_id)?;
    Some(ShipTerminalSnapshot {
        output: terminal.output.borrow().clone(),
        truncated: false,
        exit: terminal
            .exit_status
            .borrow()
            .as_ref()
            .map(map_terminal_exit_status),
    })
}

fn map_terminal_exit_status(exit_status: &TerminalExitStatus) -> ShipTerminalExit {
    ShipTerminalExit {
        exit_code: exit_status.exit_code,
        signal: exit_status.signal.clone(),
    }
}

fn map_json_value(value: &serde_json::Value) -> ShipJsonValue {
    match value {
        serde_json::Value::Null => ShipJsonValue::Null,
        serde_json::Value::Bool(value) => ShipJsonValue::Bool { value: *value },
        serde_json::Value::Number(value) => ShipJsonValue::Number {
            value: value.to_string(),
        },
        serde_json::Value::String(value) => ShipJsonValue::String {
            value: value.clone(),
        },
        serde_json::Value::Array(items) => ShipJsonValue::Array {
            items: items.iter().map(map_json_value).collect(),
        },
        serde_json::Value::Object(entries) => ShipJsonValue::Object {
            entries: entries
                .iter()
                .map(|(key, value)| JsonEntry {
                    key: key.clone(),
                    value: map_json_value(value),
                })
                .collect(),
        },
    }
}

fn map_permission_options(
    options: &[agent_client_protocol::PermissionOption],
) -> Vec<PermissionOption> {
    options
        .iter()
        .map(|option| PermissionOption {
            option_id: option.option_id.0.to_string(),
            label: option.name.clone(),
            kind: map_permission_option_kind(option.kind),
        })
        .collect()
}

fn map_permission_option_kind(kind: PermissionOptionKind) -> ShipPermissionOptionKind {
    match kind {
        PermissionOptionKind::AllowOnce => ShipPermissionOptionKind::AllowOnce,
        PermissionOptionKind::AllowAlways => ShipPermissionOptionKind::AllowAlways,
        PermissionOptionKind::RejectOnce => ShipPermissionOptionKind::RejectOnce,
        PermissionOptionKind::RejectAlways => ShipPermissionOptionKind::RejectAlways,
        _ => ShipPermissionOptionKind::Other,
    }
}

fn display_path_for_path(worktree_path: &Path, path: &Path) -> Option<String> {
    if path.as_os_str().is_empty() {
        return None;
    }

    if let Ok(relative) = path.strip_prefix(worktree_path) {
        return Some(relative.display().to_string());
    }

    Some(path.display().to_string())
}

fn display_path_for_string(worktree_path: &Path, path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    Some(display_path_for_path(worktree_path, Path::new(path)).unwrap_or_else(|| path.to_owned()))
}

fn extract_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let candidate = extract_value(value, keys)?;
    match candidate {
        serde_json::Value::String(text) => Some(text.clone()),
        _ => None,
    }
}

fn extract_string_array(value: &serde_json::Value, keys: &[&str]) -> Option<Vec<String>> {
    let candidate = extract_value(value, keys)?;
    let items = candidate
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(ToOwned::to_owned))
        .collect::<Option<Vec<_>>>()?;
    Some(items)
}

fn extract_u32(value: &serde_json::Value, keys: &[&str]) -> Option<u32> {
    let candidate = extract_value(value, keys)?;
    candidate
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
}

fn extract_value<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    let object = value.as_object()?;
    keys.iter().find_map(|key| object.get(*key))
}

fn map_plan_status(status: agent_client_protocol::PlanEntryStatus) -> PlanStepStatus {
    match status {
        agent_client_protocol::PlanEntryStatus::Pending => PlanStepStatus::Pending,
        agent_client_protocol::PlanEntryStatus::InProgress => PlanStepStatus::InProgress,
        agent_client_protocol::PlanEntryStatus::Completed => PlanStepStatus::Completed,
        _ => PlanStepStatus::Failed,
    }
}

fn remaining_context_percent(used: u64, size: u64) -> Option<u8> {
    if size == 0 {
        return None;
    }

    let remaining = size.saturating_sub(used);
    let percent = remaining.saturating_mul(100) / size;
    Some(percent.min(u8::MAX as u64) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{
        ContentChunk, PermissionOption as AcpPermissionOption, Plan, PlanEntry, PlanEntryPriority,
        PlanEntryStatus, RequestPermissionRequest, SessionNotification, Terminal, TextContent,
        ToolCallLocation, ToolCallUpdate, ToolCallUpdateFields, UsageUpdate,
    };
    use std::process::Stdio;
    use tokio::process::Command;

    fn make_client() -> ShipAcpClient {
        let (tx, _rx) = mpsc::unbounded_channel();
        ShipAcpClient::new(Role::Mate, PathBuf::from("/tmp/worktree"), tx)
    }

    // r[verify acp.client.session-notification]
    // r[verify event.context-updated]
    #[test]
    fn usage_update_json_decodes_and_maps_to_context_updated() {
        let notification: SessionNotification = serde_json::from_str(
            r#"{
                "sessionId":"01J00000000000000000000000",
                "update":{"sessionUpdate":"usage_update","used":25,"size":100}
            }"#,
        )
        .expect("usage_update should decode");

        let client = make_client();
        let events = client.map_session_update(notification.update);

        assert_eq!(
            events,
            vec![SessionEvent::ContextUpdated {
                role: Role::Mate,
                remaining_percent: 75,
            }]
        );
    }

    // r[verify event.block-id.text]
    #[test]
    fn consecutive_text_chunks_reuse_block_id_and_append() {
        let client = make_client();
        client.begin_prompt_turn();

        let first = client.map_session_update(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            ContentBlock::Text(TextContent::new("Hello".to_owned())),
        )));
        let second = client.map_session_update(SessionUpdate::AgentMessageChunk(
            ContentChunk::new(ContentBlock::Text(TextContent::new(", world".to_owned()))),
        ));

        let block_id = match &first[..] {
            [SessionEvent::BlockAppend { block_id, .. }] => block_id.clone(),
            other => panic!("unexpected first events: {other:?}"),
        };

        assert_eq!(
            second,
            vec![SessionEvent::BlockPatch {
                block_id,
                role: Role::Mate,
                patch: BlockPatch::TextAppend {
                    text: ", world".to_owned(),
                },
            }]
        );
    }

    // r[verify event.block-id.text]
    #[test]
    fn usage_update_does_not_break_text_chunk_accumulation() {
        let client = make_client();
        client.begin_prompt_turn();

        let first = client.map_session_update(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            ContentBlock::Text(TextContent::new("Hello".to_owned())),
        )));
        let _ = client.map_session_update(SessionUpdate::UsageUpdate(UsageUpdate::new(10, 100)));
        let second = client.map_session_update(SessionUpdate::AgentMessageChunk(
            ContentChunk::new(ContentBlock::Text(TextContent::new("Again".to_owned()))),
        ));

        let first_id = match &first[..] {
            [SessionEvent::BlockAppend { block_id, .. }] => block_id.clone(),
            other => panic!("unexpected first events: {other:?}"),
        };
        assert_eq!(
            second,
            vec![SessionEvent::BlockPatch {
                block_id: first_id,
                role: Role::Mate,
                patch: BlockPatch::TextAppend {
                    text: "Again".to_owned(),
                },
            }]
        );
    }

    #[test]
    fn repeated_tool_call_updates_reuse_the_same_block_id() {
        let client = make_client();
        let tool_call_id = "toolu_123".to_owned();

        let first = client.map_session_update(SessionUpdate::ToolCall(
            agent_client_protocol::ToolCall::new(
                agent_client_protocol::ToolCallId::new(tool_call_id.clone()),
                "Read File",
            )
            .status(ToolCallStatus::InProgress),
        ));
        let second = client.map_session_update(SessionUpdate::ToolCall(
            agent_client_protocol::ToolCall::new(
                agent_client_protocol::ToolCallId::new(tool_call_id.clone()),
                "Read File",
            )
            .status(ToolCallStatus::Completed),
        ));

        assert!(matches!(
            first.as_slice(),
            [SessionEvent::BlockAppend { .. }]
        ));
        let block_id = match first.as_slice() {
            [
                SessionEvent::BlockAppend {
                    block_id,
                    block:
                        ShipContentBlock::ToolCall {
                            tool_call_id: Some(mapped_tool_call_id),
                            ..
                        },
                    ..
                },
            ] => {
                assert_eq!(mapped_tool_call_id, "toolu_123");
                block_id.clone()
            }
            other => panic!("unexpected first events: {other:?}"),
        };
        assert_eq!(
            second,
            vec![SessionEvent::BlockPatch {
                block_id,
                role: Role::Mate,
                patch: BlockPatch::ToolCallUpdate {
                    tool_name: Some("Read File".to_owned()),
                    kind: Some(ShipToolCallKind::Other),
                    target: None,
                    raw_input: None,
                    raw_output: None,
                    status: ShipToolCallStatus::Success,
                    locations: Some(Vec::new()),
                    content: Some(Vec::new()),
                    error: None,
                },
            }]
        );
    }

    // r[verify acp.content-blocks]
    // r[verify acp.terminals]
    #[tokio::test(flavor = "current_thread")]
    async fn tool_calls_map_structured_targets_and_terminal_snapshots() {
        let client = make_client();
        let terminal_id = "terminal-1".to_owned();
        let child = Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("test process should spawn");
        let exit_status = TerminalExitStatus::new().exit_code(0_u32);
        let (_tx, exit_status_rx) = watch::channel(Some(exit_status.clone()));
        client.terminals.borrow_mut().insert(
            terminal_id.clone(),
            TerminalState {
                child: Rc::new(RefCell::new(child)),
                output: Rc::new(RefCell::new("checked output".to_owned())),
                exit_status: Rc::new(RefCell::new(Some(exit_status))),
                exit_status_rx,
            },
        );

        let events = client.map_session_update(SessionUpdate::ToolCall(
            agent_client_protocol::ToolCall::new(
                agent_client_protocol::ToolCallId::new("toolu_exec"),
                "Bash",
            )
            .kind(ToolKind::Execute)
            .status(ToolCallStatus::Completed)
            .raw_input(serde_json::json!({
                "command": "rg",
                "args": ["PermissionBlock", "frontend/src"],
                "cwd": "/tmp/worktree/frontend",
            }))
            .content(vec![ToolCallContent::Terminal(Terminal::new(terminal_id))]),
        ));

        let [SessionEvent::BlockAppend { block, .. }] = events.as_slice() else {
            panic!("unexpected events: {events:?}");
        };
        let ShipContentBlock::ToolCall {
            tool_call_id,
            kind,
            target,
            content,
            ..
        } = block
        else {
            panic!("expected tool call block: {block:?}");
        };

        assert_eq!(tool_call_id.as_deref(), Some("toolu_exec"));
        assert_eq!(*kind, Some(ShipToolCallKind::Execute));
        assert_eq!(
            target,
            &Some(ShipToolTarget::Command {
                command: "rg PermissionBlock frontend/src".to_owned(),
                cwd: Some("/tmp/worktree/frontend".to_owned()),
                display_cwd: Some("frontend".to_owned()),
            })
        );
        assert_eq!(
            content,
            &vec![ShipToolCallContent::Terminal {
                terminal_id: "terminal-1".to_owned(),
                snapshot: Some(ShipTerminalSnapshot {
                    output: "checked output".to_owned(),
                    truncated: false,
                    exit: Some(ShipTerminalExit {
                        exit_code: Some(0),
                        signal: None,
                    }),
                }),
            }]
        );
    }

    // r[verify captain.capabilities]
    #[tokio::test(flavor = "current_thread")]
    async fn captain_builtin_tool_requests_are_silently_rejected() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (tx, rx) = mpsc::unbounded_channel();
                let client = Rc::new(ShipAcpClient::new(
                    Role::Captain,
                    PathBuf::from("/tmp/worktree"),
                    tx,
                ));

                // Edit on a code file → blocked
                let request = RequestPermissionRequest::new(
                    "session-1",
                    ToolCallUpdate::new(
                        "toolu_perm",
                        ToolCallUpdateFields::new()
                            .title("Write File".to_owned())
                            .kind(ToolKind::Edit)
                            .locations(vec![ToolCallLocation::new("/tmp/worktree/src/lib.rs")])
                            .raw_input(serde_json::json!({
                                "path": "/tmp/worktree/src/lib.rs",
                            })),
                    ),
                    vec![
                        AcpPermissionOption::new(
                            "allow-always",
                            "Allow always",
                            PermissionOptionKind::AllowAlways,
                        ),
                        AcpPermissionOption::new(
                            "reject-once",
                            "Reject once",
                            PermissionOptionKind::RejectOnce,
                        ),
                    ],
                );

                let response = client
                    .request_permission(request)
                    .await
                    .expect("permission request should succeed");

                assert_eq!(
                    response.outcome,
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                        "reject-once",
                    ))
                );

                // No events should be emitted for auto-rejected permissions.
                assert!(rx.is_empty());
            })
            .await;
    }

    // r[verify mate.capabilities]
    #[tokio::test(flavor = "current_thread")]
    async fn mate_write_requests_are_rejected() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (tx, rx) = mpsc::unbounded_channel();
                let client = Rc::new(ShipAcpClient::new(
                    Role::Mate,
                    PathBuf::from("/tmp/worktree"),
                    tx,
                ));

                // Edit on a code file → blocked
                let request = RequestPermissionRequest::new(
                    "session-1",
                    ToolCallUpdate::new(
                        "toolu_perm_blocked",
                        ToolCallUpdateFields::new()
                            .title("Write File".to_owned())
                            .kind(ToolKind::Edit)
                            .locations(vec![ToolCallLocation::new("/tmp/worktree/src/lib.rs")])
                            .raw_input(serde_json::json!({
                                "path": "/tmp/worktree/src/lib.rs",
                            })),
                    ),
                    vec![
                        AcpPermissionOption::new(
                            "allow-once",
                            "Allow once",
                            PermissionOptionKind::AllowOnce,
                        ),
                        AcpPermissionOption::new(
                            "reject-once",
                            "Reject once",
                            PermissionOptionKind::RejectOnce,
                        ),
                    ],
                );

                let response = client
                    .request_permission(request)
                    .await
                    .expect("permission request should succeed");

                assert_eq!(
                    response.outcome,
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                        "reject-once",
                    ))
                );

                assert!(rx.is_empty());
            })
            .await;
    }

    // r[verify mate.capabilities]
    #[tokio::test(flavor = "current_thread")]
    async fn non_write_permissions_are_auto_approved() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (tx, rx) = mpsc::unbounded_channel();
                let client = Rc::new(ShipAcpClient::new(
                    Role::Mate,
                    PathBuf::from("/tmp/worktree"),
                    tx,
                ));

                let request = RequestPermissionRequest::new(
                    "session-1",
                    ToolCallUpdate::new(
                        "toolu_plan",
                        ToolCallUpdateFields::new()
                            .title("ExitPlanMode".to_owned())
                            .kind(ToolKind::Think),
                    ),
                    vec![
                        AcpPermissionOption::new(
                            "allow-once",
                            "Allow once",
                            PermissionOptionKind::AllowOnce,
                        ),
                        AcpPermissionOption::new(
                            "reject-once",
                            "Reject once",
                            PermissionOptionKind::RejectOnce,
                        ),
                    ],
                );

                let response = client
                    .request_permission(request)
                    .await
                    .expect("permission request should succeed");

                assert_eq!(
                    response.outcome,
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                        "allow-once",
                    ))
                );

                assert!(rx.is_empty());
            })
            .await;
    }

    // r[verify mate.capabilities]
    #[tokio::test(flavor = "current_thread")]
    async fn mate_read_builtin_is_auto_approved() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (tx, rx) = mpsc::unbounded_channel();
                let client = Rc::new(ShipAcpClient::new(
                    Role::Mate,
                    PathBuf::from("/tmp/worktree"),
                    tx,
                ));

                let request = RequestPermissionRequest::new(
                    "session-1",
                    ToolCallUpdate::new(
                        "toolu_read",
                        ToolCallUpdateFields::new()
                            .title("Read File".to_owned())
                            .kind(ToolKind::Read)
                            .raw_input(serde_json::json!({
                                "path": "/tmp/worktree/src/lib.rs",
                            })),
                    ),
                    vec![
                        AcpPermissionOption::new(
                            "allow-once",
                            "Allow once",
                            PermissionOptionKind::AllowOnce,
                        ),
                        AcpPermissionOption::new(
                            "reject-once",
                            "Reject once",
                            PermissionOptionKind::RejectOnce,
                        ),
                    ],
                );

                let response = client
                    .request_permission(request)
                    .await
                    .expect("permission request should succeed");

                assert_eq!(
                    response.outcome,
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                        "allow-once",
                    ))
                );

                assert!(rx.is_empty());
            })
            .await;
    }

    // r[verify acp.plans]
    // r[verify agent-state.plan-step]
    #[test]
    fn plan_updates_map_priority_and_status() {
        let client = make_client();
        let events = client.map_session_update(SessionUpdate::Plan(Plan::new(vec![
            PlanEntry::new(
                "Fix blocking bug",
                PlanEntryPriority::High,
                PlanEntryStatus::Pending,
            ),
            PlanEntry::new(
                "Refresh snapshots",
                PlanEntryPriority::Medium,
                PlanEntryStatus::InProgress,
            ),
            PlanEntry::new(
                "Polish docs",
                PlanEntryPriority::Low,
                PlanEntryStatus::Completed,
            ),
        ])));

        assert_eq!(
            events,
            vec![SessionEvent::AgentStateChanged {
                role: Role::Mate,
                state: AgentState::Working {
                    plan: Some(vec![
                        PlanStep {
                            title: String::new(),
                            description: "Fix blocking bug".to_owned(),
                            status: PlanStepStatus::Pending,
                        },
                        PlanStep {
                            title: String::new(),
                            description: "Refresh snapshots".to_owned(),
                            status: PlanStepStatus::InProgress,
                        },
                        PlanStep {
                            title: String::new(),
                            description: "Polish docs".to_owned(),
                            status: PlanStepStatus::Completed,
                        },
                    ]),
                    activity: Some("ACP plan update".to_owned()),
                },
            }]
        );
    }
}
