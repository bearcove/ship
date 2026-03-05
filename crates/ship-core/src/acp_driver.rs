use std::collections::HashMap;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_client_protocol::{
    Agent, CancelNotification, Client, ClientCapabilities, ClientSideConnection, ContentBlock,
    Error, FileSystemCapability, Implementation, InitializeRequest, NewSessionRequest,
    PlanEntryStatus, PromptRequest, ProtocolVersion, RequestPermissionRequest,
    RequestPermissionResponse, Result as AcpResult, SessionNotification, SessionUpdate,
    StopReason as AcpStopReason, TextContent, ToolCallStatus,
};
use futures::{Stream, stream};
use ship_types::{
    AgentState, BlockId, BlockPatch, ContentBlock as ShipContentBlock, PlanStep, PlanStepStatus,
    Role, SessionEvent, ToolCallStatus as ShipToolCallStatus,
};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::{AgentDriver, AgentError, AgentHandle, PromptResponse, SessionId, StopReason};

struct AcpHandle {
    command_tx: mpsc::UnboundedSender<DriverCommand>,
    notifications_rx: Arc<Mutex<mpsc::UnboundedReceiver<SessionEvent>>>,
    worker_thread: Option<std::thread::JoinHandle<()>>,
}

enum DriverCommand {
    Prompt {
        content: String,
        response: oneshot::Sender<Result<PromptResponse, AgentError>>,
    },
    Cancel {
        response: oneshot::Sender<Result<(), AgentError>>,
    },
    Kill {
        response: oneshot::Sender<Result<(), AgentError>>,
    },
}

#[derive(Default)]
pub struct AcpAgentDriver {
    handles: Mutex<HashMap<AgentHandle, AcpHandle>>,
}

impl AcpAgentDriver {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AgentDriver for AcpAgentDriver {
    // r[acp.binary.claude]
    // r[acp.binary.codex]
    // r[acp.spawn.stdio]
    // r[acp.spawn.cwd]
    // r[acp.spawn.kill-on-drop]
    // r[acp.conn.client-side]
    // r[acp.conn.local-set]
    // r[acp.conn.initialize]
    // r[acp.conn.new-session]
    async fn spawn(
        &self,
        kind: ship_types::AgentKind,
        role: Role,
        worktree_path: &Path,
    ) -> Result<AgentHandle, AgentError> {
        let handle = AgentHandle::new(SessionId::new());
        let (command_tx, command_rx) = mpsc::unbounded_channel::<DriverCommand>();
        let (notifications_tx, notifications_rx) = mpsc::unbounded_channel::<SessionEvent>();
        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();

        let worktree_path = worktree_path.to_path_buf();
        let worker_thread = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready_tx.send(Err(format!("failed to create ACP runtime: {error}")));
                    return;
                }
            };

            let local_set = tokio::task::LocalSet::new();
            runtime.block_on(local_set.run_until(async move {
                if let Err(error) = run_acp_worker(
                    kind,
                    role,
                    worktree_path,
                    command_rx,
                    notifications_tx,
                    ready_tx,
                )
                .await
                {
                    tracing::warn!(%error, "acp worker exited with error");
                }
            }));
        });

        match ready_rx.await {
            Ok(Ok(())) => {
                self.handles
                    .lock()
                    .expect("acp handles mutex poisoned")
                    .insert(
                        handle.clone(),
                        AcpHandle {
                            command_tx,
                            notifications_rx: Arc::new(Mutex::new(notifications_rx)),
                            worker_thread: Some(worker_thread),
                        },
                    );
                Ok(handle)
            }
            Ok(Err(message)) => Err(AgentError { message }),
            Err(error) => Err(AgentError {
                message: format!("acp worker setup channel failed: {error}"),
            }),
        }
    }

    async fn prompt(
        &self,
        handle: &AgentHandle,
        content: &str,
    ) -> Result<PromptResponse, AgentError> {
        let command_tx = {
            let handles = self.handles.lock().expect("acp handles mutex poisoned");
            handles
                .get(handle)
                .ok_or_else(|| AgentError {
                    message: "agent handle not found".to_owned(),
                })?
                .command_tx
                .clone()
        };

        let (response_tx, response_rx) = oneshot::channel();
        command_tx
            .send(DriverCommand::Prompt {
                content: content.to_owned(),
                response: response_tx,
            })
            .map_err(|error| AgentError {
                message: format!("failed to send prompt command: {error}"),
            })?;

        response_rx.await.map_err(|error| AgentError {
            message: format!("prompt response channel closed: {error}"),
        })?
    }

    async fn cancel(&self, handle: &AgentHandle) -> Result<(), AgentError> {
        let command_tx = {
            let handles = self.handles.lock().expect("acp handles mutex poisoned");
            handles
                .get(handle)
                .ok_or_else(|| AgentError {
                    message: "agent handle not found".to_owned(),
                })?
                .command_tx
                .clone()
        };

        let (response_tx, response_rx) = oneshot::channel();
        command_tx
            .send(DriverCommand::Cancel {
                response: response_tx,
            })
            .map_err(|error| AgentError {
                message: format!("failed to send cancel command: {error}"),
            })?;

        response_rx.await.map_err(|error| AgentError {
            message: format!("cancel response channel closed: {error}"),
        })?
    }

    fn notifications(
        &self,
        handle: &AgentHandle,
    ) -> Pin<Box<dyn Stream<Item = SessionEvent> + Send + '_>> {
        let notifications_rx = {
            let handles = self.handles.lock().expect("acp handles mutex poisoned");
            handles.get(handle).map(|acp| acp.notifications_rx.clone())
        };

        let Some(notifications_rx) = notifications_rx else {
            return Box::pin(stream::iter(Vec::<SessionEvent>::new()));
        };

        let mut out = Vec::new();
        let mut rx = notifications_rx
            .lock()
            .expect("acp notifications mutex poisoned");
        while let Ok(event) = rx.try_recv() {
            out.push(event);
        }

        Box::pin(stream::iter(out))
    }

    async fn kill(&self, handle: &AgentHandle) -> Result<(), AgentError> {
        let mut acp_handle = self
            .handles
            .lock()
            .expect("acp handles mutex poisoned")
            .remove(handle)
            .ok_or_else(|| AgentError {
                message: "agent handle not found".to_owned(),
            })?;

        let (response_tx, response_rx) = oneshot::channel();
        acp_handle
            .command_tx
            .send(DriverCommand::Kill {
                response: response_tx,
            })
            .map_err(|error| AgentError {
                message: format!("failed to send kill command: {error}"),
            })?;

        let result = response_rx.await.map_err(|error| AgentError {
            message: format!("kill response channel closed: {error}"),
        })?;

        if let Some(join_handle) = acp_handle.worker_thread.take() {
            let _ = join_handle.join();
        }

        result
    }
}

async fn run_acp_worker(
    kind: ship_types::AgentKind,
    role: Role,
    worktree_path: std::path::PathBuf,
    mut command_rx: mpsc::UnboundedReceiver<DriverCommand>,
    notifications_tx: mpsc::UnboundedSender<SessionEvent>,
    ready_tx: oneshot::Sender<Result<(), String>>,
) -> Result<(), AgentError> {
    let mut command = match kind {
        ship_types::AgentKind::Claude => {
            let mut command = Command::new("npx");
            command.arg("@zed-industries/claude-agent-acp");
            command
        }
        ship_types::AgentKind::Codex => Command::new("codex-acp"),
    };

    let mut child = command
        .current_dir(&worktree_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| AgentError {
            message: format!("failed to spawn ACP process: {error}"),
        })?;

    let child_stdin = child.stdin.take().ok_or_else(|| AgentError {
        message: "failed to capture ACP stdin".to_owned(),
    })?;
    let child_stdout = child.stdout.take().ok_or_else(|| AgentError {
        message: "failed to capture ACP stdout".to_owned(),
    })?;

    let client = StubClient {
        role,
        notifications_tx,
    };

    let (connection, io_task) = ClientSideConnection::new(
        client,
        child_stdin.compat_write(),
        child_stdout.compat(),
        |future| {
            tokio::task::spawn_local(future);
        },
    );

    tokio::task::spawn_local(async move {
        if let Err(error) = io_task.await {
            tracing::warn!(%error, "acp io task failed");
        }
    });

    let initialize_request = InitializeRequest::new(ProtocolVersion::LATEST)
        .client_info(Implementation::new("ship", env!("CARGO_PKG_VERSION")))
        .client_capabilities(
            ClientCapabilities::new()
                .terminal(true)
                .fs(FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(true)),
        );

    connection
        .initialize(initialize_request)
        .await
        .map_err(acp_error)?;

    let session_id = connection
        .new_session(NewSessionRequest::new(worktree_path).mcp_servers(vec![]))
        .await
        .map_err(acp_error)?
        .session_id;

    let _ = ready_tx.send(Ok(()));

    while let Some(command) = command_rx.recv().await {
        match command {
            DriverCommand::Prompt { content, response } => {
                let result = connection
                    .prompt(PromptRequest::new(
                        session_id.clone(),
                        vec![ContentBlock::Text(TextContent::new(content))],
                    ))
                    .await
                    .map(map_prompt_response)
                    .map_err(acp_error);
                let _ = response.send(result);
            }
            DriverCommand::Cancel { response } => {
                let result = connection
                    .cancel(CancelNotification::new(session_id.clone()))
                    .await
                    .map_err(acp_error)
                    .map(|_| ());
                let _ = response.send(result);
            }
            DriverCommand::Kill { response } => {
                let result = child.start_kill().map_err(|error| AgentError {
                    message: format!("failed to kill ACP process: {error}"),
                });
                let _ = child.wait().await;
                let _ = response.send(result.map(|_| ()));
                return Ok(());
            }
        }
    }

    let _ = child.start_kill();
    let _ = child.wait().await;
    Ok(())
}

fn map_prompt_response(response: agent_client_protocol::PromptResponse) -> PromptResponse {
    PromptResponse {
        stop_reason: match response.stop_reason {
            AcpStopReason::EndTurn => StopReason::EndTurn,
            AcpStopReason::Cancelled => StopReason::Cancelled,
            AcpStopReason::MaxTokens | AcpStopReason::MaxTurnRequests => {
                StopReason::ContextExhausted
            }
            AcpStopReason::Refusal => StopReason::EndTurn,
            _ => StopReason::EndTurn,
        },
    }
}

fn acp_error(error: Error) -> AgentError {
    AgentError {
        message: error.to_string(),
    }
}

struct StubClient {
    role: Role,
    notifications_tx: mpsc::UnboundedSender<SessionEvent>,
}

#[async_trait::async_trait(?Send)]
impl Client for StubClient {
    async fn request_permission(
        &self,
        _args: RequestPermissionRequest,
    ) -> AcpResult<RequestPermissionResponse> {
        Err(Error::method_not_found())
    }

    async fn session_notification(&self, args: SessionNotification) -> AcpResult<()> {
        for event in map_session_update(self.role, args.update) {
            let _ = self.notifications_tx.send(event);
        }
        Ok(())
    }
}

fn map_session_update(role: Role, update: SessionUpdate) -> Vec<SessionEvent> {
    match update {
        SessionUpdate::UserMessageChunk(chunk)
        | SessionUpdate::AgentMessageChunk(chunk)
        | SessionUpdate::AgentThoughtChunk(chunk) => {
            vec![SessionEvent::BlockAppend {
                block_id: BlockId::new(),
                role,
                block: ShipContentBlock::Text {
                    text: format_content_block(chunk.content),
                },
            }]
        }
        SessionUpdate::ToolCall(tool_call) => {
            let result_text = if tool_call.content.is_empty() {
                None
            } else {
                Some(format!("{:#?}", tool_call.content))
            };
            vec![SessionEvent::BlockAppend {
                block_id: BlockId(tool_call.tool_call_id.0.to_string()),
                role,
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
        SessionUpdate::ToolCallUpdate(update) => vec![SessionEvent::BlockPatch {
            block_id: BlockId(update.tool_call_id.0.to_string()),
            role,
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
        }],
        SessionUpdate::Plan(plan) => vec![SessionEvent::AgentStateChanged {
            role,
            state: AgentState::Working {
                plan: Some(
                    plan.entries
                        .into_iter()
                        .map(|entry| PlanStep {
                            description: entry.content,
                            status: match entry.status {
                                PlanEntryStatus::Pending => PlanStepStatus::Planned,
                                PlanEntryStatus::InProgress => PlanStepStatus::InProgress,
                                PlanEntryStatus::Completed => PlanStepStatus::Completed,
                                _ => PlanStepStatus::Failed,
                            },
                        })
                        .collect(),
                ),
                activity: Some("ACP plan update".to_owned()),
            },
        }],
        _ => Vec::new(),
    }
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
