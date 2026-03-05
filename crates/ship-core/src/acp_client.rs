use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::ExitStatus;
use std::rc::Rc;
use std::time::Duration;

use agent_client_protocol::{
    Client, ContentBlock, CreateTerminalRequest, CreateTerminalResponse, Error,
    KillTerminalCommandRequest, KillTerminalCommandResponse, PermissionOptionKind,
    ReadTextFileRequest, ReadTextFileResponse, ReleaseTerminalRequest, ReleaseTerminalResponse,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    Result as AcpResult, SelectedPermissionOutcome, SessionNotification, SessionUpdate,
    TerminalExitStatus, TerminalId, TerminalOutputRequest, TerminalOutputResponse, ToolCallStatus,
    WaitForTerminalExitRequest, WaitForTerminalExitResponse, WriteTextFileRequest,
    WriteTextFileResponse,
};
use ship_types::{
    AgentState, BlockId, BlockPatch, ContentBlock as ShipContentBlock, PermissionRequest, PlanStep,
    PlanStepStatus, Role, SessionEvent, ToolCallStatus as ShipToolCallStatus,
};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, watch};

pub struct ShipAcpClient {
    role: Role,
    worktree_path: PathBuf,
    notifications_tx: mpsc::UnboundedSender<SessionEvent>,
    active_text_block: Rc<RefCell<Option<BlockId>>>,
    pending_permissions: Rc<RefCell<HashMap<String, oneshot::Sender<bool>>>>,
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
            pending_permissions: Rc::new(RefCell::new(HashMap::new())),
            terminals: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn begin_prompt_turn(&self) {
        self.reset_text_block();
    }

    pub fn resolve_permission(&self, permission_id: &str, approved: bool) -> Result<(), String> {
        let sender = self
            .pending_permissions
            .borrow_mut()
            .remove(permission_id)
            .ok_or_else(|| format!("pending permission not found: {permission_id}"))?;
        sender
            .send(approved)
            .map_err(|_| format!("permission receiver dropped: {permission_id}"))
    }

    fn send_event(&self, event: SessionEvent) {
        let _ = self.notifications_tx.send(event);
    }

    fn reset_text_block(&self) {
        *self.active_text_block.borrow_mut() = None;
    }

    fn map_session_update(&self, update: SessionUpdate) -> Vec<SessionEvent> {
        match update {
            SessionUpdate::UserMessageChunk(chunk)
            | SessionUpdate::AgentMessageChunk(chunk)
            | SessionUpdate::AgentThoughtChunk(chunk) => {
                let text = format_content_block(chunk.content);
                let existing_block = self.active_text_block.borrow().clone();
                match existing_block {
                    Some(block_id) => vec![SessionEvent::BlockPatch {
                        // r[event.block-id.text]
                        block_id,
                        role: self.role,
                        patch: BlockPatch::TextAppend { text },
                    }],
                    None => {
                        let block_id = BlockId::new();
                        *self.active_text_block.borrow_mut() = Some(block_id.clone());
                        vec![SessionEvent::BlockAppend {
                            // r[event.block-id.text]
                            block_id,
                            role: self.role,
                            block: ShipContentBlock::Text { text },
                        }]
                    }
                }
            }
            SessionUpdate::ToolCall(tool_call) => {
                self.reset_text_block();
                let result_text = if tool_call.content.is_empty() {
                    None
                } else {
                    Some(format!("{:#?}", tool_call.content))
                };
                vec![SessionEvent::BlockAppend {
                    block_id: BlockId(tool_call.tool_call_id.0.to_string()),
                    role: self.role,
                    block: ShipContentBlock::ToolCall {
                        tool_name: tool_call.title,
                        arguments: tool_call
                            .raw_input
                            .map(|value| value.to_string())
                            .unwrap_or_default(),
                        status: map_tool_status(tool_call.status),
                        result: result_text,
                    },
                }]
            }
            SessionUpdate::ToolCallUpdate(update) => {
                self.reset_text_block();
                vec![SessionEvent::BlockPatch {
                    block_id: BlockId(update.tool_call_id.0.to_string()),
                    role: self.role,
                    patch: BlockPatch::ToolCallUpdate {
                        status: update
                            .fields
                            .status
                            .map(map_tool_status)
                            .unwrap_or(ShipToolCallStatus::Running),
                        result: update
                            .fields
                            .content
                            .map(|content| format!("{:#?}", content)),
                    },
                }]
            }
            SessionUpdate::Plan(plan) => {
                self.reset_text_block();
                vec![SessionEvent::AgentStateChanged {
                    role: self.role,
                    state: AgentState::Working {
                        plan: Some(
                            plan.entries
                                .into_iter()
                                .map(|entry| PlanStep {
                                    description: entry.content,
                                    status: match entry.status {
                                        agent_client_protocol::PlanEntryStatus::Pending => {
                                            PlanStepStatus::Planned
                                        }
                                        agent_client_protocol::PlanEntryStatus::InProgress => {
                                            PlanStepStatus::InProgress
                                        }
                                        agent_client_protocol::PlanEntryStatus::Completed => {
                                            PlanStepStatus::Completed
                                        }
                                        _ => PlanStepStatus::Failed,
                                    },
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
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> AcpResult<RequestPermissionResponse> {
        self.reset_text_block();
        let permission_id = args.tool_call.tool_call_id.0.to_string();
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

        self.send_event(SessionEvent::BlockAppend {
            block_id: BlockId(permission_id.clone()),
            role: self.role,
            block: ShipContentBlock::Permission {
                tool_name: tool_name.clone(),
                description: description.clone(),
                arguments: arguments.clone(),
                resolution: None,
            },
        });
        self.send_event(SessionEvent::AgentStateChanged {
            role: self.role,
            state: AgentState::AwaitingPermission {
                request: PermissionRequest {
                    permission_id: permission_id.clone(),
                    tool_name,
                    arguments,
                    description,
                },
            },
        });

        let (resolution_tx, resolution_rx) = oneshot::channel();
        self.pending_permissions
            .borrow_mut()
            .insert(permission_id.clone(), resolution_tx);

        let approved = resolution_rx.await.unwrap_or(false);

        self.send_event(SessionEvent::AgentStateChanged {
            role: self.role,
            state: AgentState::Working {
                plan: None,
                activity: Some("Permission resolved".to_owned()),
            },
        });

        let approved_option = args
            .options
            .iter()
            .find(|option| {
                matches!(
                    option.kind,
                    PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
                )
            })
            .map(|option| option.option_id.clone());
        let denied_option = args
            .options
            .iter()
            .find(|option| {
                matches!(
                    option.kind,
                    PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways
                )
            })
            .map(|option| option.option_id.clone());
        let fallback_option = args.options.first().map(|option| option.option_id.clone());

        let chosen = if approved {
            approved_option.or(fallback_option)
        } else {
            denied_option.or(fallback_option)
        };

        let outcome = match chosen {
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
    async fn write_text_file(
        &self,
        args: WriteTextFileRequest,
    ) -> AcpResult<WriteTextFileResponse> {
        let path = resolve_relative_to_worktree(&self.worktree_path, &args.path);
        tokio::fs::write(path, args.content)
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(WriteTextFileResponse::new())
    }

    // r[acp.client.fs-read]
    async fn read_text_file(&self, args: ReadTextFileRequest) -> AcpResult<ReadTextFileResponse> {
        let path = resolve_relative_to_worktree(&self.worktree_path, &args.path);
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(ReadTextFileResponse::new(content))
    }

    // r[acp.client.terminal-create]
    async fn create_terminal(
        &self,
        args: CreateTerminalRequest,
    ) -> AcpResult<CreateTerminalResponse> {
        let terminal_id = ulid::Ulid::new().to_string();
        let mut command = Command::new(&args.command);
        command
            .args(args.args)
            .current_dir(&self.worktree_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for env_var in args.env {
            command.env(env_var.name, env_var.value);
        }

        let mut child = command.spawn().map_err(|_| Error::internal_error())?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let output = Rc::new(RefCell::new(String::new()));
        let exit_status = Rc::new(RefCell::new(None));
        let (exit_status_tx, exit_status_rx) = watch::channel(None::<TerminalExitStatus>);
        let child = Rc::new(RefCell::new(child));

        if let Some(stdout) = stdout {
            let output_ref = output.clone();
            tokio::task::spawn_local(async move {
                read_terminal_stream(stdout, output_ref).await;
            });
        }
        if let Some(stderr) = stderr {
            let output_ref = output.clone();
            tokio::task::spawn_local(async move {
                read_terminal_stream(stderr, output_ref).await;
            });
        }

        {
            let child_ref = child.clone();
            let exit_status_ref = exit_status.clone();
            tokio::task::spawn_local(async move {
                watch_terminal_exit(child_ref, exit_status_ref, exit_status_tx).await;
            });
        }

        self.terminals.borrow_mut().insert(
            terminal_id.clone(),
            TerminalState {
                child,
                output,
                exit_status,
                exit_status_rx,
            },
        );

        Ok(CreateTerminalResponse::new(TerminalId::new(terminal_id)))
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

async fn read_terminal_stream<R: AsyncRead + Unpin>(mut stream: R, output: Rc<RefCell<String>>) {
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => return,
            Ok(read) => {
                output
                    .borrow_mut()
                    .push_str(&String::from_utf8_lossy(&buffer[..read]));
            }
            Err(error) => {
                tracing::warn!(%error, "failed reading terminal output");
                return;
            }
        }
    }
}

async fn watch_terminal_exit(
    child: Rc<RefCell<Child>>,
    exit_status: Rc<RefCell<Option<TerminalExitStatus>>>,
    exit_status_tx: watch::Sender<Option<TerminalExitStatus>>,
) {
    loop {
        let exited = {
            let mut child = child.borrow_mut();
            match child.try_wait() {
                Ok(status) => status,
                Err(error) => {
                    tracing::warn!(%error, "failed to check terminal exit status");
                    let failed = TerminalExitStatus::new().signal("unknown".to_owned());
                    *exit_status.borrow_mut() = Some(failed.clone());
                    let _ = exit_status_tx.send(Some(failed));
                    return;
                }
            }
        };

        if let Some(status) = exited {
            let terminal_status = map_exit_status(status);
            *exit_status.borrow_mut() = Some(terminal_status.clone());
            let _ = exit_status_tx.send(Some(terminal_status));
            return;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn kill_if_running(child: &Rc<RefCell<Child>>) -> std::io::Result<()> {
    let mut child = child.borrow_mut();
    if child.try_wait()?.is_none() {
        child.start_kill()?;
    }
    Ok(())
}

fn map_exit_status(exit_status: ExitStatus) -> TerminalExitStatus {
    let mut status =
        TerminalExitStatus::new().exit_code(exit_status.code().map(|code| code as u32));
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = exit_status.signal() {
            status = status.signal(signal.to_string());
        }
    }
    status
}

fn resolve_relative_to_worktree(worktree_path: &Path, path: &Path) -> PathBuf {
    let mut resolved = PathBuf::from(worktree_path);
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(part) => resolved.push(part),
        }
    }
    resolved
}

fn format_content_block(block: ContentBlock) -> String {
    match block {
        ContentBlock::Text(text) => text.text,
        other => format!("{other:?}"),
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
    use agent_client_protocol::{ContentChunk, SessionNotification, TextContent, UsageUpdate};

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
}
