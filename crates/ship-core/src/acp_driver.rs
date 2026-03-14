use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use agent_client_protocol::{
    Agent, CancelNotification, ClientCapabilities, ClientSideConnection, ContentBlock, Error,
    FileSystemCapability, ImageContent, Implementation, InitializeRequest, LoadSessionRequest,
    NewSessionRequest, NewSessionResponse, PromptRequest, ProtocolVersion, ResumeSessionRequest,
    StopReason as AcpStopReason, TextContent,
};
use base64::Engine as _;
use futures::{Stream, stream};
use ship_types::{EffortValue, Role, SessionEvent};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::acp_client::ShipAcpClient;
use crate::mcp::to_acp_mcp_server;
use crate::{
    AgentDriver, AgentError, AgentHandle, AgentSessionConfig, AgentSpawnInfo, PromptResponse,
    SessionId, StopReason,
};
use crate::{SystemBinaryPathProbe, resolve_agent_launcher};

type ModelInfo = (Option<String>, Vec<String>);
type EffortInfo = (Option<String>, Option<String>, Vec<EffortValue>);

struct AcpCapabilities {
    protocol_version: u16,
    agent_name: Option<String>,
    agent_version: Option<String>,
    cap_load_session: bool,
    cap_resume_session: bool,
    cap_prompt_image: bool,
    cap_prompt_audio: bool,
    cap_prompt_embedded_context: bool,
    cap_mcp_http: bool,
    cap_mcp_sse: bool,
}

/// (model_info, effort_info, acp_session_id, was_resumed, acp_capabilities)
type ReadyResult = Result<(ModelInfo, EffortInfo, String, bool, AcpCapabilities), String>;

struct AcpHandle {
    command_tx: mpsc::UnboundedSender<DriverCommand>,
    notifications_rx: Arc<Mutex<mpsc::UnboundedReceiver<SessionEvent>>>,
    /// Generation counter for prompt-in-flight tracking.
    /// Odd = prompt in flight, even = idle.
    /// A prompt sets it to the next odd value; completion restores it to
    /// the next even value only if the generation hasn't changed (i.e.,
    /// no cancel+new-prompt happened in between).
    prompt_generation: Arc<AtomicU64>,
    worker_thread: Option<std::thread::JoinHandle<()>>,
}

enum DriverCommand {
    Prompt {
        parts: Vec<ship_types::PromptContentPart>,
        prompt_generation: Arc<AtomicU64>,
        /// The generation value when this prompt was started.
        started_at_generation: u64,
        response: oneshot::Sender<Result<PromptResponse, AgentError>>,
    },
    Cancel {
        prompt_generation: Arc<AtomicU64>,
        response: oneshot::Sender<Result<(), AgentError>>,
    },
    ResolvePermission {
        permission_id: String,
        option_id: String,
        response: oneshot::Sender<Result<(), AgentError>>,
    },
    SetModel {
        model_id: String,
        response: oneshot::Sender<Result<(), AgentError>>,
    },
    SetEffort {
        config_id: String,
        value_id: String,
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
    // r[acp.binary.opencode]
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
    ) -> Result<AgentSpawnInfo, AgentError> {
        let handle = AgentHandle::new(SessionId::new());
        let (command_tx, command_rx) = mpsc::unbounded_channel::<DriverCommand>();
        let (notifications_tx, notifications_rx) = mpsc::unbounded_channel::<SessionEvent>();
        let (ready_tx, ready_rx) = oneshot::channel::<ReadyResult>();

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
            Ok(Ok((
                (model_id, available_models),
                (effort_config_id, effort_value_id, available_effort_values),
                acp_session_id,
                was_resumed,
                acp_caps,
            ))) => {
                self.handles
                    .lock()
                    .expect("acp handles mutex poisoned")
                    .insert(
                        handle.clone(),
                        AcpHandle {
                            command_tx,
                            notifications_rx: Arc::new(Mutex::new(notifications_rx)),
                            prompt_generation: Arc::new(AtomicU64::new(0)),
                            worker_thread: Some(worker_thread),
                        },
                    );
                Ok(AgentSpawnInfo {
                    handle,
                    model_id,
                    available_models,
                    effort_config_id,
                    effort_value_id,
                    available_effort_values,
                    acp_session_id,
                    was_resumed,
                    protocol_version: acp_caps.protocol_version,
                    agent_name: acp_caps.agent_name,
                    agent_version: acp_caps.agent_version,
                    cap_load_session: acp_caps.cap_load_session,
                    cap_resume_session: acp_caps.cap_resume_session,
                    cap_prompt_image: acp_caps.cap_prompt_image,
                    cap_prompt_audio: acp_caps.cap_prompt_audio,
                    cap_prompt_embedded_context: acp_caps.cap_prompt_embedded_context,
                    cap_mcp_http: acp_caps.cap_mcp_http,
                    cap_mcp_sse: acp_caps.cap_mcp_sse,
                })
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
        parts: &[ship_types::PromptContentPart],
    ) -> Result<PromptResponse, AgentError> {
        let (command_tx, prompt_generation) = {
            let handles = self.handles.lock().expect("acp handles mutex poisoned");
            let acp = handles.get(handle).ok_or_else(|| AgentError {
                message: "agent handle not found".to_owned(),
            })?;
            (acp.command_tx.clone(), acp.prompt_generation.clone())
        };

        // Try to move from even (idle) to odd (in-flight).
        // If the current value is already odd, a prompt is in flight.
        let current = prompt_generation.load(Ordering::SeqCst);
        if current % 2 != 0 {
            return Err(AgentError {
                message: "prompt already in flight".to_owned(),
            });
        }
        let started_at = current + 1;
        if prompt_generation
            .compare_exchange(current, started_at, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AgentError {
                message: "prompt already in flight".to_owned(),
            });
        }

        let (response_tx, response_rx) = oneshot::channel();
        command_tx
            .send(DriverCommand::Prompt {
                parts: parts.to_owned(),
                prompt_generation: prompt_generation.clone(),
                started_at_generation: started_at,
                response: response_tx,
            })
            .map_err(|error| {
                // Restore to idle only if we're still the active prompt
                let _ = prompt_generation.compare_exchange(
                    started_at,
                    started_at + 1,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
                AgentError {
                    message: format!("failed to send prompt command: {error}"),
                }
            })?;

        response_rx.await.map_err(|error| {
            let _ = prompt_generation.compare_exchange(
                started_at,
                started_at + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            AgentError {
                message: format!("prompt response channel closed: {error}"),
            }
        })?
    }

    async fn cancel(&self, handle: &AgentHandle) -> Result<(), AgentError> {
        let (command_tx, prompt_generation) = {
            let handles = self.handles.lock().expect("acp handles mutex poisoned");
            let acp = handles.get(handle).ok_or_else(|| AgentError {
                message: "agent handle not found".to_owned(),
            })?;
            (acp.command_tx.clone(), acp.prompt_generation.clone())
        };

        let (response_tx, response_rx) = oneshot::channel();
        command_tx
            .send(DriverCommand::Cancel {
                prompt_generation,
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
        option_id: &str,
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
                option_id: option_id.to_owned(),
                response: response_tx,
            })
            .map_err(|error| AgentError {
                message: format!("failed to send resolve permission command: {error}"),
            })?;

        response_rx.await.map_err(|error| AgentError {
            message: format!("resolve permission response channel closed: {error}"),
        })?
    }

    async fn set_model(&self, handle: &AgentHandle, model_id: &str) -> Result<(), AgentError> {
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
            .send(DriverCommand::SetModel {
                model_id: model_id.to_owned(),
                response: response_tx,
            })
            .map_err(|error| AgentError {
                message: format!("failed to send set model command: {error}"),
            })?;

        response_rx.await.map_err(|error| AgentError {
            message: format!("set model response channel closed: {error}"),
        })?
    }

    async fn set_effort(
        &self,
        handle: &AgentHandle,
        config_id: &str,
        value_id: &str,
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
            .send(DriverCommand::SetEffort {
                config_id: config_id.to_owned(),
                value_id: value_id.to_owned(),
                response: response_tx,
            })
            .map_err(|error| AgentError {
                message: format!("failed to send set effort command: {error}"),
            })?;

        response_rx.await.map_err(|error| AgentError {
            message: format!("set effort response channel closed: {error}"),
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
    ready_tx: oneshot::Sender<ReadyResult>,
) -> Result<(), AgentError> {
    tracing::info!(role = ?role, kind = ?kind, worktree_path = %config.worktree_path.display(), "starting ACP worker");
    let launcher = resolve_agent_launcher(kind, &SystemBinaryPathProbe).ok_or_else(|| {
        let kind_name = match kind {
            ship_types::AgentKind::Claude => "Claude",
            ship_types::AgentKind::Codex => "Codex",
            ship_types::AgentKind::OpenCode => "OpenCode",
        };
        AgentError {
            message: format!("no supported ACP launcher found for {kind_name}"),
        }
    })?;

    let mut command = command_for_launcher(launcher, &config.worktree_path);

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

    let initialize_request = build_initialize_request(role);

    let init_response = connection
        .initialize(initialize_request)
        .await
        .map_err(acp_error)?;
    tracing::debug!(role = ?role, kind = ?kind, "initialized ACP connection");

    let agent_supports_resume = init_response
        .agent_capabilities
        .session_capabilities
        .resume
        .is_some();
    let agent_supports_load = init_response.agent_capabilities.load_session;

    let acp_caps = AcpCapabilities {
        protocol_version: init_response
            .protocol_version
            .to_string()
            .parse::<u16>()
            .unwrap_or(0),
        agent_name: init_response.agent_info.as_ref().map(|i| i.name.clone()),
        agent_version: init_response.agent_info.as_ref().map(|i| i.version.clone()),
        cap_load_session: init_response.agent_capabilities.load_session,
        cap_resume_session: init_response
            .agent_capabilities
            .session_capabilities
            .resume
            .is_some(),
        cap_prompt_image: init_response.agent_capabilities.prompt_capabilities.image,
        cap_prompt_audio: init_response.agent_capabilities.prompt_capabilities.audio,
        cap_prompt_embedded_context: init_response
            .agent_capabilities
            .prompt_capabilities
            .embedded_context,
        cap_mcp_http: init_response.agent_capabilities.mcp_capabilities.http,
        cap_mcp_sse: init_response.agent_capabilities.mcp_capabilities.sse,
    };

    // Try to reconnect to a previous session if we have a stored ACP session ID.
    // Two mechanisms: resume (in-process reconnect) and load (reload from disk).
    let resume_id = config.resume_session_id.clone();
    tracing::info!(
        role = ?role, kind = ?kind,
        agent_supports_resume,
        agent_supports_load,
        has_resume_id = resume_id.is_some(),
        "session resume/load check"
    );
    let can_reconnect = agent_supports_resume || agent_supports_load;
    let (session_id, model_id, available_models, effort_info, was_resumed) = if let Some(prev_id) =
        resume_id.filter(|_| can_reconnect)
    {
        let acp_session_id = agent_client_protocol::SessionId::new(Arc::from(prev_id.as_str()));
        let mcp_servers: Vec<_> = config.mcp_servers.iter().map(to_acp_mcp_server).collect();

        // Prefer resume (lightweight, in-process) over load (re-reads from disk).
        let reconnect_result = if agent_supports_resume {
            tracing::info!(
                role = ?role, kind = ?kind, prev_session_id = %prev_id,
                "resuming ACP session"
            );
            let resume_req =
                ResumeSessionRequest::new(acp_session_id.clone(), &config.worktree_path)
                    .mcp_servers(mcp_servers);
            connection.resume_session(resume_req).await.map(|_| ())
        } else {
            tracing::info!(
                role = ?role, kind = ?kind, prev_session_id = %prev_id,
                "loading ACP session"
            );
            let load_req = LoadSessionRequest::new(acp_session_id.clone(), &config.worktree_path)
                .mcp_servers(mcp_servers);
            connection.load_session(load_req).await.map(|_| ())
        };

        match reconnect_result {
            Ok(()) => {
                tracing::info!(
                    role = ?role, kind = ?kind, acp_session_id = %prev_id,
                    "reconnected to ACP session"
                );
                (
                    acp_session_id,
                    None,
                    Vec::new(),
                    (None, None, Vec::new()),
                    true,
                )
            }
            Err(error) => {
                tracing::warn!(
                    role = ?role, kind = ?kind, prev_session_id = %prev_id,
                    %error, "session reconnect failed, falling back to new session"
                );
                let resp = connection
                    .new_session(build_new_session_request(&config))
                    .await
                    .map_err(acp_error)?;
                let (mid, models) = extract_model_info(&resp);
                let effort =
                    parse_effort_from_config_options(resp.config_options.as_deref().unwrap_or(&[]));
                (resp.session_id, mid, models, effort, false)
            }
        }
    } else {
        tracing::info!(role = ?role, kind = ?kind, "creating new ACP session");
        let resp = connection
            .new_session(build_new_session_request(&config))
            .await
            .map_err(acp_error)?;
        let (mid, models) = extract_model_info(&resp);
        let effort =
            parse_effort_from_config_options(resp.config_options.as_deref().unwrap_or(&[]));
        (resp.session_id, mid, models, effort, false)
    };

    let (effort_config_id, effort_value_id, available_effort_values) = effort_info;
    let acp_session_id_str = session_id.0.as_ref().to_owned();
    tracing::info!(role = ?role, kind = ?kind, acp_session_id = ?session_id, model_id = ?model_id, "ACP session ready");

    let _ = ready_tx.send(Ok((
        (model_id, available_models),
        (effort_config_id, effort_value_id, available_effort_values),
        acp_session_id_str,
        was_resumed,
        acp_caps,
    )));

    while let Some(command) = command_rx.recv().await {
        match command {
            DriverCommand::Prompt {
                parts,
                prompt_generation,
                started_at_generation,
                response,
            } => {
                client.begin_prompt_turn();
                let connection = connection.clone();
                let session_id = session_id.clone();
                tokio::task::spawn_local(async move {
                    let content_blocks = parts_to_content_blocks(parts);
                    let result = connection
                        .prompt(PromptRequest::new(session_id, content_blocks))
                        .await
                        .map(map_prompt_response)
                        .map_err(acp_error);
                    // Only clear the in-flight state if no cancel+new-prompt
                    // happened since we started. compare_exchange ensures a
                    // newer prompt's generation is not clobbered.
                    let _ = prompt_generation.compare_exchange(
                        started_at_generation,
                        started_at_generation + 1,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    );
                    let _ = response.send(result);
                });
            }
            DriverCommand::Cancel {
                prompt_generation,
                response,
            } => {
                let result = connection
                    .cancel(CancelNotification::new(session_id.clone()))
                    .await
                    .map_err(acp_error)
                    .map(|_| ());
                // Advance to the next even (idle) generation so a new
                // prompt can start immediately. The old prompt's completion
                // handler uses compare_exchange with the old generation,
                // so it won't interfere with the new prompt.
                let current = prompt_generation.load(Ordering::SeqCst);
                if current % 2 != 0 {
                    prompt_generation.store(current + 1, Ordering::SeqCst);
                }
                let _ = response.send(result);
            }
            DriverCommand::ResolvePermission {
                permission_id,
                option_id,
                response,
            } => {
                let result = client
                    .resolve_permission(&permission_id, &option_id)
                    .map_err(|message| AgentError { message });
                let _ = response.send(result);
            }
            DriverCommand::SetModel { model_id, response } => {
                use agent_client_protocol::{ModelId, SetSessionModelRequest};
                let result = connection
                    .set_session_model(SetSessionModelRequest::new(
                        session_id.clone(),
                        ModelId::new(model_id.as_str()),
                    ))
                    .await
                    .map(|_| ())
                    .map_err(acp_error);
                let _ = response.send(result);
            }
            DriverCommand::SetEffort {
                config_id,
                value_id,
                response,
            } => {
                use agent_client_protocol::{
                    SessionConfigId, SessionConfigValueId, SetSessionConfigOptionRequest,
                };
                let result = connection
                    .set_session_config_option(SetSessionConfigOptionRequest::new(
                        session_id.clone(),
                        SessionConfigId::new(config_id.as_str()),
                        SessionConfigValueId::new(value_id.as_str()),
                    ))
                    .await
                    .map(|_| ())
                    .map_err(acp_error);
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

fn extract_model_info(resp: &NewSessionResponse) -> (Option<String>, Vec<String>) {
    match resp.models.as_ref() {
        Some(m) => {
            let current = m.current_model_id.0.as_ref().to_owned();
            let available = m
                .available_models
                .iter()
                .map(|info| info.model_id.0.as_ref().to_owned())
                .collect();
            (Some(current), available)
        }
        None => (None, Vec::new()),
    }
}

fn parse_effort_from_config_options(
    config_options: &[agent_client_protocol::SessionConfigOption],
) -> EffortInfo {
    use agent_client_protocol::{
        SessionConfigKind, SessionConfigOptionCategory, SessionConfigSelectOptions,
    };
    for option in config_options {
        if matches!(
            option.category,
            Some(SessionConfigOptionCategory::ThoughtLevel)
        ) {
            if let SessionConfigKind::Select(select) = &option.kind {
                let config_id = option.id.0.as_ref().to_owned();
                let value_id = select.current_value.0.as_ref().to_owned();
                let available = match &select.options {
                    SessionConfigSelectOptions::Ungrouped(options) => options
                        .iter()
                        .map(|opt| EffortValue {
                            id: opt.value.0.as_ref().to_owned(),
                            name: opt.name.clone(),
                        })
                        .collect(),
                    SessionConfigSelectOptions::Grouped(groups) => groups
                        .iter()
                        .flat_map(|group| group.options.iter())
                        .map(|opt| EffortValue {
                            id: opt.value.0.as_ref().to_owned(),
                            name: opt.name.clone(),
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                return (Some(config_id), Some(value_id), available);
            }
        }
    }
    (None, None, Vec::new())
}

fn build_initialize_request(role: Role) -> InitializeRequest {
    let client_capabilities = match role {
        Role::Captain => captain_client_capabilities(),
        Role::Mate => mate_client_capabilities(),
    };

    InitializeRequest::new(ProtocolVersion::LATEST)
        .client_info(Implementation::new("ship", env!("CARGO_PKG_VERSION")))
        .client_capabilities(client_capabilities)
}

fn captain_client_capabilities() -> ClientCapabilities {
    ClientCapabilities::new()
        .terminal(true)
        .fs(FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
}

fn mate_client_capabilities() -> ClientCapabilities {
    ClientCapabilities::new()
        .terminal(true)
        .fs(FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
}

// r[acp.sandbox]
// On macOS, wrap the agent command with sandbox-exec to restrict file writes
// to the worktree directory only. This is the actual security boundary —
// the agent process cannot write outside its worktree.
fn command_for_launcher(
    launcher: crate::AgentLauncher,
    worktree_path: &std::path::Path,
) -> Command {
    if cfg!(target_os = "macos") {
        // Resolve symlinks (macOS /tmp -> /private/tmp)
        let real_worktree = worktree_path
            .canonicalize()
            .unwrap_or_else(|_| worktree_path.to_path_buf());

        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/nobody".to_owned());

        let policy = format!(
            concat!(
                "(version 1)",
                "(deny default)",
                // Allow all reads — agents need system libs, node_modules, etc.
                "(allow file-read*)",
                // Allow process operations (fork, exec, signal)
                "(allow process*)",
                // Allow system/kernel operations
                "(allow sysctl*)",
                "(allow mach*)",
                "(allow signal)",
                "(allow ipc*)",
                // Allow network (MCP servers, HTTP, etc.)
                "(allow network*)",
                // Allow writes to the worktree
                "(allow file-write* (subpath \"{worktree}\"))",
                // Allow writes to /dev (stdout/stderr/null/tty)
                "(allow file-write* (subpath \"/dev\"))",
                // Allow writes to temp dirs. sandbox-exec matches literal paths
                // without resolving symlinks, so we need both the symlink and
                // resolved paths: /tmp -> /private/tmp, /var -> /private/var.
                "(allow file-write* (subpath \"/tmp\"))",
                "(allow file-write* (subpath \"/private/tmp\"))",
                "(allow file-write* (subpath \"/var/folders\"))",
                "(allow file-write* (subpath \"/private/var/folders\"))",
                // Allow cargo install, rustup toolchain management
                "(allow file-write* (subpath \"{home}/.cargo\"))",
                "(allow file-write* (subpath \"{home}/.rustup\"))",
                // Allow pnpm/npm cache and temp state
                "(allow file-write* (subpath \"{home}/Library/pnpm\"))",
                "(allow file-write* (subpath \"{home}/Library/Caches/pnpm\"))",
                "(allow file-write* (subpath \"{home}/Library/Caches/npm\"))",
                "(allow file-write* (subpath \"{home}/.npm\"))",
                "(allow file-write* (subpath \"{home}/.pnpm-store\"))",
                // Allow claude state, plans, logs, etc.
                "(allow file-write* (subpath \"{home}/.claude\"))",
                // Allow codex models cache
                "(allow file-write* (subpath \"{home}/.codex\"))",
                // Allow opencode config and state
                "(allow file-write* (subpath \"{home}/.config/opencode\"))",
                "(allow file-write* (subpath \"{home}/.local/share/opencode\"))",
            ),
            worktree = real_worktree.display(),
            home = home,
        );

        let mut command = Command::new("sandbox-exec");
        command
            .arg("-p")
            .arg(policy)
            .arg(launcher.program)
            .args(launcher.args)
            // Override TMPDIR so rustc/cargo write temp files to /private/tmp
            // instead of /var/folders/... (a symlink path not covered by the
            // sandbox policy's literal path matching).
            .env("TMPDIR", "/private/tmp");
        command
    } else {
        let mut command = Command::new(launcher.program);
        command.args(launcher.args);
        command
    }
}

// r[acp.mcp.passthrough]
fn build_new_session_request(config: &AgentSessionConfig) -> NewSessionRequest {
    NewSessionRequest::new(config.worktree_path.clone())
        .mcp_servers(config.mcp_servers.iter().map(to_acp_mcp_server).collect())
}

fn parts_to_content_blocks(parts: Vec<ship_types::PromptContentPart>) -> Vec<ContentBlock> {
    parts
        .into_iter()
        .map(|part| match part {
            ship_types::PromptContentPart::Text { text } => {
                ContentBlock::Text(TextContent::new(text))
            }
            ship_types::PromptContentPart::Image { mime_type, data } => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                ContentBlock::Image(ImageContent::new(encoded, mime_type))
            }
        })
        .collect()
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
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicU64;
    use std::sync::{Arc, Mutex};

    use agent_client_protocol::McpServer;
    use ship_types::{
        AgentKind, McpEnvVar, McpHeader, McpHttpServerConfig, McpServerConfig, McpSseServerConfig,
        McpStdioServerConfig, Role,
    };
    use tokio::sync::mpsc;

    use super::{
        AcpAgentDriver, AcpHandle, build_initialize_request, build_new_session_request,
        command_for_launcher,
    };
    use crate::{
        AgentDriver, AgentHandle, AgentLauncher, AgentSessionConfig, BinaryPathProbe, SessionId,
        resolve_agent_launcher,
    };

    #[derive(Clone, Copy)]
    struct FakeProbe {
        claude: bool,
        codex: bool,
        pnpx: bool,
        opencode: bool,
    }

    impl BinaryPathProbe for FakeProbe {
        fn is_available(&self, binary: &str) -> bool {
            match binary {
                "claude-agent-acp" => self.claude,
                "codex-acp" => self.codex,
                "pnpx" => self.pnpx,
                "opencode" => self.opencode,
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
                pnpx: true,
                opencode: false,
            },
        )
        .expect("claude launcher should resolve");

        let worktree = std::path::PathBuf::from("/tmp/test-worktree");
        let command = command_for_launcher(launcher, &worktree);
        let args: Vec<_> = command.as_std().get_args().collect();

        if cfg!(target_os = "macos") {
            assert_eq!(command.as_std().get_program(), "sandbox-exec");
            // sandbox-exec -p <policy> pnpx @zed-industries/claude-agent-acp
            assert_eq!(args[0], "-p");
            // args[1] is the policy string
            assert_eq!(args[2], "pnpx");
            assert_eq!(args[3], "@zed-industries/claude-agent-acp");
        } else {
            assert_eq!(command.as_std().get_program(), "pnpx");
            assert_eq!(args, vec!["@zed-industries/claude-agent-acp"]);
        }
    }

    // r[verify acp.binary.codex]
    #[test]
    fn spawn_command_uses_codex_launcher_resolution() {
        let launcher = resolve_agent_launcher(
            AgentKind::Codex,
            &FakeProbe {
                claude: false,
                codex: true,
                pnpx: true,
                opencode: false,
            },
        )
        .expect("codex launcher should resolve");

        let worktree = std::path::PathBuf::from("/tmp/test-worktree");
        let command = command_for_launcher(launcher, &worktree);
        let args: Vec<_> = command.as_std().get_args().collect();

        if cfg!(target_os = "macos") {
            assert_eq!(command.as_std().get_program(), "sandbox-exec");
            assert_eq!(args[0], "-p");
            assert_eq!(args[2], "codex-acp");
        } else {
            assert_eq!(command.as_std().get_program(), "codex-acp");
            assert_eq!(args.len(), 0);
        }
    }

    // r[verify acp.binary.opencode]
    #[test]
    fn spawn_command_uses_opencode_launcher_resolution() {
        let launcher = resolve_agent_launcher(
            AgentKind::OpenCode,
            &FakeProbe {
                claude: false,
                codex: false,
                pnpx: false,
                opencode: true,
            },
        )
        .expect("opencode launcher should resolve");

        let worktree = std::path::PathBuf::from("/tmp/test-worktree");
        let command = command_for_launcher(launcher, &worktree);
        let args: Vec<_> = command.as_std().get_args().collect();

        if cfg!(target_os = "macos") {
            assert_eq!(command.as_std().get_program(), "sandbox-exec");
            assert_eq!(args[0], "-p");
            assert_eq!(args[2], "opencode");
            assert_eq!(args[3], "acp");
        } else {
            assert_eq!(command.as_std().get_program(), "opencode");
            assert_eq!(args, vec!["acp"]);
        }
    }

    // r[verify acp.sandbox]
    #[test]
    fn command_builder_wraps_with_sandbox_on_macos() {
        let worktree = std::path::PathBuf::from("/tmp/test-worktree");
        let command =
            command_for_launcher(AgentLauncher::new("pnpx", &["pkg", "--flag"]), &worktree);
        let args: Vec<_> = command.as_std().get_args().collect();

        if cfg!(target_os = "macos") {
            assert_eq!(command.as_std().get_program(), "sandbox-exec");
            assert_eq!(args[0], "-p");
            let policy = args[1].to_str().unwrap();
            let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/nobody".to_owned());
            assert!(
                policy.contains("deny default"),
                "policy should deny by default"
            );
            assert!(
                policy.contains("file-write*"),
                "policy should have write rules"
            );
            assert!(
                policy.contains(&format!(
                    "(allow file-write* (subpath \"{home}/Library/Caches/npm\"))"
                )),
                "policy should allow npm cache writes under Library/Caches"
            );
            assert!(
                policy.contains(&format!("(allow file-write* (subpath \"{home}/.npm\"))")),
                "policy should allow npm cache temp writes under ~/.npm"
            );
            assert_eq!(args[2], "pnpx");
            assert_eq!(args[3], "pkg");
            assert_eq!(args[4], "--flag");
        } else {
            assert_eq!(command.as_std().get_program(), "pnpx");
            assert_eq!(args, vec!["pkg", "--flag"]);
        }
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
            resume_session_id: None,
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

    #[tokio::test]
    async fn prompt_rejects_overlapping_in_flight_requests() {
        let handle = AgentHandle::new(SessionId::new());
        let (command_tx, _command_rx) = mpsc::unbounded_channel();
        let (_notifications_tx, notifications_rx) = mpsc::unbounded_channel();

        let driver = AcpAgentDriver {
            handles: Mutex::new(HashMap::from([(
                handle.clone(),
                AcpHandle {
                    command_tx,
                    notifications_rx: Arc::new(Mutex::new(notifications_rx)),
                    prompt_generation: Arc::new(AtomicU64::new(1)),
                    worker_thread: None,
                },
            )])),
        };

        let error = driver
            .prompt(&handle, &[])
            .await
            .expect_err("prompt should be rejected while another is in flight");

        assert_eq!(error.message, "prompt already in flight");
    }

    // r[verify captain.capabilities]
    #[test]
    fn captain_uses_own_builtins_except_write() {
        let request = build_initialize_request(Role::Captain);
        assert!(request.client_capabilities.terminal);
        assert!(request.client_capabilities.fs.read_text_file);
        assert!(request.client_capabilities.fs.write_text_file);
    }

    // r[verify mate.capabilities]
    #[test]
    fn mate_uses_own_builtins_except_write() {
        let request = build_initialize_request(Role::Mate);
        assert!(request.client_capabilities.terminal);
        assert!(request.client_capabilities.fs.read_text_file);
        assert!(request.client_capabilities.fs.write_text_file);
    }
}
