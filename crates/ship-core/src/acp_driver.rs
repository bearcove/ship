use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use agent_client_protocol::{
    Agent, CancelNotification, ClientCapabilities, ClientSideConnection, ContentBlock, Error,
    FileSystemCapability, Implementation, InitializeRequest, NewSessionRequest, PromptRequest,
    ProtocolVersion, StopReason as AcpStopReason, TextContent,
};
use futures::{Stream, stream};
use ship_types::{Role, SessionEvent};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::acp_client::ShipAcpClient;
use crate::mcp::to_acp_mcp_server;
use crate::{
    AgentDriver, AgentError, AgentHandle, AgentSessionConfig, PromptResponse, SessionId, StopReason,
};
use crate::{SystemBinaryPathProbe, resolve_agent_launcher};

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
    ResolvePermission {
        permission_id: String,
        approved: bool,
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
        config: &AgentSessionConfig,
    ) -> Result<AgentHandle, AgentError> {
        let handle = AgentHandle::new(SessionId::new());
        let (command_tx, command_rx) = mpsc::unbounded_channel::<DriverCommand>();
        let (notifications_tx, notifications_rx) = mpsc::unbounded_channel::<SessionEvent>();
        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();

        let config = config.clone();
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
                if let Err(error) =
                    run_acp_worker(kind, role, config, command_rx, notifications_tx, ready_tx).await
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

    async fn resolve_permission(
        &self,
        handle: &AgentHandle,
        permission_id: &str,
        approved: bool,
    ) -> Result<(), AgentError> {
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
            .send(DriverCommand::ResolvePermission {
                permission_id: permission_id.to_owned(),
                approved,
                response: response_tx,
            })
            .map_err(|error| AgentError {
                message: format!("failed to send resolve permission command: {error}"),
            })?;

        response_rx.await.map_err(|error| AgentError {
            message: format!("resolve permission response channel closed: {error}"),
        })?
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
    config: AgentSessionConfig,
    mut command_rx: mpsc::UnboundedReceiver<DriverCommand>,
    notifications_tx: mpsc::UnboundedSender<SessionEvent>,
    ready_tx: oneshot::Sender<Result<(), String>>,
) -> Result<(), AgentError> {
    tracing::info!(role = ?role, kind = ?kind, worktree_path = %config.worktree_path.display(), "starting ACP worker");
    let launcher = resolve_agent_launcher(kind, &SystemBinaryPathProbe).ok_or_else(|| {
        let kind_name = match kind {
            ship_types::AgentKind::Claude => "Claude",
            ship_types::AgentKind::Codex => "Codex",
        };
        AgentError {
            message: format!("no supported ACP launcher found for {kind_name}"),
        }
    })?;

    let mut command = command_for_launcher(launcher);

    let mut child = command
        .current_dir(&config.worktree_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| AgentError {
            message: format!("failed to spawn ACP process: {error}"),
        })?;
    tracing::info!(role = ?role, kind = ?kind, worktree_path = %config.worktree_path.display(), "spawned ACP process");

    let child_stdin = child.stdin.take().ok_or_else(|| AgentError {
        message: "failed to capture ACP stdin".to_owned(),
    })?;
    let child_stdout = child.stdout.take().ok_or_else(|| AgentError {
        message: "failed to capture ACP stdout".to_owned(),
    })?;

    let client = Rc::new(ShipAcpClient::new(
        role,
        config.worktree_path.clone(),
        notifications_tx,
    ));

    let (connection, io_task) = ClientSideConnection::new(
        client.clone(),
        child_stdin.compat_write(),
        child_stdout.compat(),
        |future| {
            tokio::task::spawn_local(future);
        },
    );
    let connection = Rc::new(connection);

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
    tracing::debug!(role = ?role, kind = ?kind, "initialized ACP connection");

    let session_id = connection
        .new_session(build_new_session_request(&config))
        .await
        .map_err(acp_error)?
        .session_id;
    tracing::info!(role = ?role, kind = ?kind, acp_session_id = ?session_id, "started ACP session");

    let _ = ready_tx.send(Ok(()));

    while let Some(command) = command_rx.recv().await {
        match command {
            DriverCommand::Prompt { content, response } => {
                client.begin_prompt_turn();
                let connection = connection.clone();
                let session_id = session_id.clone();
                tokio::task::spawn_local(async move {
                    let result = connection
                        .prompt(PromptRequest::new(
                            session_id,
                            vec![ContentBlock::Text(TextContent::new(content))],
                        ))
                        .await
                        .map(map_prompt_response)
                        .map_err(acp_error);
                    let _ = response.send(result);
                });
            }
            DriverCommand::Cancel { response } => {
                let result = connection
                    .cancel(CancelNotification::new(session_id.clone()))
                    .await
                    .map_err(acp_error)
                    .map(|_| ());
                let _ = response.send(result);
            }
            DriverCommand::ResolvePermission {
                permission_id,
                approved,
                response,
            } => {
                let result = client
                    .resolve_permission(&permission_id, approved)
                    .map_err(|message| AgentError { message });
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

fn command_for_launcher(launcher: crate::AgentLauncher) -> Command {
    let mut command = Command::new(launcher.program);
    command.args(launcher.args);
    command
}

// r[acp.mcp.passthrough]
fn build_new_session_request(config: &AgentSessionConfig) -> NewSessionRequest {
    NewSessionRequest::new(config.worktree_path.clone())
        .mcp_servers(config.mcp_servers.iter().map(to_acp_mcp_server).collect())
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use agent_client_protocol::McpServer;
    use ship_types::{
        AgentKind, McpEnvVar, McpHeader, McpHttpServerConfig, McpServerConfig, McpSseServerConfig,
        McpStdioServerConfig,
    };

    use super::{build_new_session_request, command_for_launcher};
    use crate::{AgentLauncher, AgentSessionConfig, BinaryPathProbe, resolve_agent_launcher};

    #[derive(Clone, Copy)]
    struct FakeProbe {
        claude: bool,
        codex: bool,
        npx: bool,
    }

    impl BinaryPathProbe for FakeProbe {
        fn is_available(&self, binary: &str) -> bool {
            match binary {
                "claude-agent-acp" => self.claude,
                "codex-acp" => self.codex,
                "npx" => self.npx,
                other => panic!("unexpected binary lookup: {other}"),
            }
        }
    }

    // r[verify acp.binary.claude]
    #[test]
    fn spawn_command_uses_claude_launcher_resolution() {
        let launcher = resolve_agent_launcher(
            AgentKind::Claude,
            &FakeProbe {
                claude: false,
                codex: false,
                npx: true,
            },
        )
        .expect("claude launcher should resolve");

        let command = command_for_launcher(launcher);

        assert_eq!(command.as_std().get_program(), "npx");
        assert_eq!(
            command.as_std().get_args().collect::<Vec<_>>(),
            vec!["@zed-industries/claude-agent-acp"]
        );
    }

    // r[verify acp.binary.codex]
    #[test]
    fn spawn_command_uses_codex_launcher_resolution() {
        let launcher = resolve_agent_launcher(
            AgentKind::Codex,
            &FakeProbe {
                claude: false,
                codex: true,
                npx: true,
            },
        )
        .expect("codex launcher should resolve");

        let command = command_for_launcher(launcher);

        assert_eq!(command.as_std().get_program(), "codex-acp");
        assert_eq!(command.as_std().get_args().count(), 0);
    }

    #[test]
    fn command_builder_preserves_launcher_program_and_args() {
        let command = command_for_launcher(AgentLauncher::new("npx", &["pkg", "--flag"]));

        assert_eq!(command.as_std().get_program(), "npx");
        assert_eq!(
            command.as_std().get_args().collect::<Vec<_>>(),
            vec!["pkg", "--flag"]
        );
    }

    // r[verify acp.mcp.passthrough]
    // r[verify acp.conn.new-session]
    #[test]
    fn new_session_request_includes_configured_mcp_servers() {
        let config = AgentSessionConfig {
            worktree_path: PathBuf::from("/repo/worktree"),
            mcp_servers: vec![
                McpServerConfig::Http(McpHttpServerConfig {
                    name: "tracey".to_owned(),
                    url: "http://127.0.0.1:9001/mcp".to_owned(),
                    headers: vec![McpHeader {
                        name: "Authorization".to_owned(),
                        value: "Bearer token".to_owned(),
                    }],
                }),
                McpServerConfig::Sse(McpSseServerConfig {
                    name: "sse".to_owned(),
                    url: "http://127.0.0.1:9002/sse".to_owned(),
                    headers: Vec::new(),
                }),
                McpServerConfig::Stdio(McpStdioServerConfig {
                    name: "filesystem".to_owned(),
                    command: "/usr/bin/fs-mcp".to_owned(),
                    args: vec!["--root".to_owned(), "/repo".to_owned()],
                    env: vec![McpEnvVar {
                        name: "HOME".to_owned(),
                        value: "/tmp/home".to_owned(),
                    }],
                }),
            ],
        };

        let request = build_new_session_request(&config);

        assert_eq!(request.cwd, PathBuf::from("/repo/worktree"));
        assert_eq!(request.mcp_servers.len(), 3);
        assert!(matches!(
            &request.mcp_servers[0],
            McpServer::Http(server)
                if server.name == "tracey"
                    && server.url == "http://127.0.0.1:9001/mcp"
                    && server.headers.len() == 1
                    && server.headers[0].name == "Authorization"
                    && server.headers[0].value == "Bearer token"
        ));
        assert!(matches!(
            &request.mcp_servers[1],
            McpServer::Sse(server)
                if server.name == "sse" && server.url == "http://127.0.0.1:9002/sse"
        ));
        assert!(matches!(
            &request.mcp_servers[2],
            McpServer::Stdio(server)
                if server.name == "filesystem"
                    && server.command == Path::new("/usr/bin/fs-mcp")
                    && server.args == vec!["--root".to_owned(), "/repo".to_owned()]
                    && server.env.len() == 1
                    && server.env[0].name == "HOME"
                    && server.env[0].value == "/tmp/home"
        ));
    }
}
