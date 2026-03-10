#[path = "agent_discovery.rs"]
mod agent_discovery;
#[path = "captain_mcp.rs"]
mod captain_mcp;
#[path = "ship_impl.rs"]
mod ship_impl;

use std::env;
use std::error::Error;
use std::time::{Duration, Instant};

use agent_discovery::{SystemBinaryPathProbe, discover_agents};
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::any;
use roam::channel;
use ship_core::ProjectRegistry;
use ship_impl::ShipImpl;
use ship_service::{ShipClient, ShipDispatcher};
use ship_types::{
    AgentKind, CreateSessionRequest, CreateSessionResponse, PromptContentPart, SessionEvent,
    SessionId, SessionStartupState, SubscribeMessage,
};
use tokio::time::{sleep, timeout};
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct ProbeState {
    ship: ShipImpl,
}

struct McpServerArgs {
    session_id: SessionId,
    server_ws_url: String,
}

enum McpServerKind {
    Captain,
    Mate,
}

struct DetectedMcpServer {
    kind: McpServerKind,
    args: McpServerArgs,
}

struct ProbeArgs {
    project: String,
    base_branch: String,
    captain_kind: AgentKind,
    mate_kind: AgentKind,
    prompt_after_ms: Option<u64>,
    prompt: Option<String>,
    idle_timeout_ms: u64,
}

impl ProbeArgs {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let Some(project) = args.next() else {
            return Err(Self::usage());
        };

        let mut parsed = Self {
            project,
            base_branch: "main".to_owned(),
            captain_kind: AgentKind::Claude,
            mate_kind: AgentKind::Claude,
            prompt_after_ms: None,
            prompt: None,
            idle_timeout_ms: 10_000,
        };

        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--base-branch" => {
                    parsed.base_branch = args
                        .next()
                        .ok_or_else(|| "--base-branch requires a value".to_owned())?;
                }
                "--captain" => {
                    parsed.captain_kind = parse_agent_kind(
                        &args
                            .next()
                            .ok_or_else(|| "--captain requires a value".to_owned())?,
                    )?;
                }
                "--mate" => {
                    parsed.mate_kind = parse_agent_kind(
                        &args
                            .next()
                            .ok_or_else(|| "--mate requires a value".to_owned())?,
                    )?;
                }
                "--prompt-after-ms" => {
                    parsed.prompt_after_ms = Some(
                        args.next()
                            .ok_or_else(|| "--prompt-after-ms requires a value".to_owned())?
                            .parse()
                            .map_err(|_| "--prompt-after-ms must be an integer".to_owned())?,
                    );
                }
                "--prompt" => {
                    parsed.prompt = Some(
                        args.next()
                            .ok_or_else(|| "--prompt requires a value".to_owned())?,
                    );
                }
                "--idle-timeout-ms" => {
                    parsed.idle_timeout_ms = args
                        .next()
                        .ok_or_else(|| "--idle-timeout-ms requires a value".to_owned())?
                        .parse()
                        .map_err(|_| "--idle-timeout-ms must be an integer".to_owned())?;
                }
                "--help" | "-h" => return Err(Self::usage()),
                other => return Err(format!("unknown flag: {other}\n\n{}", Self::usage())),
            }
        }

        if parsed.prompt_after_ms.is_some() && parsed.prompt.is_none() {
            return Err("--prompt-after-ms requires --prompt".to_owned());
        }

        Ok(parsed)
    }

    fn usage() -> String {
        "usage: cargo run -p ship-server --bin ship-startup-probe -- <project> [--base-branch main] [--captain claude|codex] [--mate claude|codex] [--prompt-after-ms 2000 --prompt \"hi\"] [--idle-timeout-ms 10000]".to_owned()
    }
}

fn parse_agent_kind(value: &str) -> Result<AgentKind, String> {
    match value.to_ascii_lowercase().as_str() {
        "claude" => Ok(AgentKind::Claude),
        "codex" => Ok(AgentKind::Codex),
        other => Err(format!("unknown agent kind: {other}")),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy()
        }))
        .init();

    if let Some(detected) = detect_mcp_server() {
        match detected.kind {
            McpServerKind::Captain => {
                captain_mcp::run_captain_stdio_server(captain_mcp::CaptainMcpServerArgs {
                    session_id: detected.args.session_id,
                    server_ws_url: detected.args.server_ws_url,
                })
                .await?;
            }
            McpServerKind::Mate => {
                captain_mcp::run_mate_stdio_server(captain_mcp::MateMcpServerArgs {
                    session_id: detected.args.session_id,
                    server_ws_url: detected.args.server_ws_url,
                })
                .await?;
            }
        }
        return Ok(());
    }

    let args = ProbeArgs::parse().map_err(|message| {
        eprintln!("{message}");
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid probe args")
    })?;

    let mut registry = ProjectRegistry::load_default().await?;
    registry.validate_all().await?;
    if registry.get(&args.project).is_none() {
        let known = registry
            .list()
            .into_iter()
            .map(|project| project.name.0)
            .collect::<Vec<_>>();
        return Err(format!(
            "project '{}' is not registered; known projects: {}",
            args.project,
            known.join(", ")
        )
        .into());
    }

    let sessions_dir = registry.config_dir().join("sessions");
    let ship = ShipImpl::new(
        registry,
        sessions_dir,
        discover_agents(&SystemBinaryPathProbe),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let ws_url = format!("ws://{}/ws", listener.local_addr()?);
    ship.set_server_ws_url(ws_url.clone());
    let app = Router::new()
        .route("/ws", any(ws_handler))
        .with_state(ProbeState { ship: ship.clone() });
    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("probe websocket server");
    });

    let ws_stream = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|error| format!("failed to connect probe websocket: {error}"))?
        .0;
    let link = roam_websocket::WsLink::new(ws_stream);
    let (client, _client_session_handle) = roam::initiator(link)
        .establish::<ShipClient>(())
        .await
        .expect("client handshake should succeed");

    let created_at = Instant::now();
    let response = client
        .create_session(CreateSessionRequest {
            project: ship_types::ProjectName(args.project.clone()),
            captain_kind: args.captain_kind,
            mate_kind: args.mate_kind,
            base_branch: args.base_branch.clone(),
            mcp_servers: None,
        })
        .await
        .map_err(|error| format!("create_session failed: {error:?}"))?;

    let session_id = match response {
        CreateSessionResponse::Created { session_id } => session_id,
        CreateSessionResponse::Failed { message } => return Err(message.into()),
    };

    println!(
        "[probe +{:>5}ms] created session {}",
        created_at.elapsed().as_millis(),
        session_id.0
    );

    let (tx, mut rx) = channel::<SubscribeMessage>();
    client
        .subscribe_events(session_id.clone(), tx)
        .await
        .map_err(|error| format!("subscribe_events failed: {error:?}"))?;

    if let (Some(delay_ms), Some(prompt)) = (args.prompt_after_ms, args.prompt.clone()) {
        let client = client.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(delay_ms)).await;
            println!(
                "[probe +{:>5}ms] sending captain prompt {:?}",
                created_at.elapsed().as_millis(),
                prompt
            );
            if let Err(error) = client
                .prompt_captain(session_id, vec![PromptContentPart::Text { text: prompt }])
                .await
            {
                eprintln!(
                    "[probe +{:>5}ms] prompt_captain failed: {:?}",
                    created_at.elapsed().as_millis(),
                    error
                );
            }
        });
    }

    loop {
        match timeout(Duration::from_millis(args.idle_timeout_ms), rx.recv()).await {
            Ok(Ok(Some(message))) => {
                log_message(created_at, &message);
            }
            Ok(Ok(None)) => {
                println!(
                    "[probe +{:>5}ms] subscription channel closed",
                    created_at.elapsed().as_millis()
                );
                break;
            }
            Ok(Err(error)) => {
                println!(
                    "[probe +{:>5}ms] subscription recv error: {}",
                    created_at.elapsed().as_millis(),
                    error
                );
                break;
            }
            Err(_) => {
                println!(
                    "[probe +{:>5}ms] no events for {}ms, stopping",
                    created_at.elapsed().as_millis(),
                    args.idle_timeout_ms
                );
                break;
            }
        }
    }

    server_task.abort();
    Ok(())
}

fn detect_mcp_server() -> Option<DetectedMcpServer> {
    let mut args = env::args().skip(1);
    let command = args.next()?;
    let kind = match command.as_str() {
        "captain-mcp-server" => McpServerKind::Captain,
        "mate-mcp-server" => McpServerKind::Mate,
        _ => return None,
    };

    let session_flag = args.next()?;
    if session_flag != "--session" {
        return None;
    }
    let session_id = SessionId(args.next()?);

    let server_ws_url_flag = args.next()?;
    if server_ws_url_flag != "--server-ws-url" {
        return None;
    }

    Some(DetectedMcpServer {
        kind,
        args: McpServerArgs {
            session_id,
            server_ws_url: args.next()?,
        },
    })
}

async fn ws_handler(State(state): State<ProbeState>, mut request: Request) -> impl IntoResponse {
    if !hyper_tungstenite::is_upgrade_request(&request) {
        return (StatusCode::BAD_REQUEST, "expected websocket upgrade").into_response();
    }

    let (response, websocket) = match hyper_tungstenite::upgrade(&mut request, None) {
        Ok(ok) => ok,
        Err(error) => {
            tracing::warn!(%error, "failed to upgrade probe websocket request");
            return (StatusCode::BAD_REQUEST, "invalid websocket upgrade").into_response();
        }
    };

    let ship = state.ship.clone();
    tokio::spawn(async move {
        let ws_stream = match websocket.await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "probe websocket upgrade future failed");
                return;
            }
        };

        let link = roam_websocket::WsLink::new(ws_stream);
        match roam::acceptor(link)
            .on_connection(ship.ship_mcp_connection_acceptor())
            .establish::<ShipClient>(ShipDispatcher::new(ship))
            .await
        {
            Ok((caller_guard, _session_handle)) => {
                let _caller_guard = caller_guard;
                std::future::pending::<()>().await;
            }
            Err(error) => {
                tracing::warn!(?error, "probe roam websocket session closed or failed");
            }
        }
    });

    response.map(Body::new).into_response()
}

fn log_message(started_at: Instant, message: &SubscribeMessage) {
    match message {
        SubscribeMessage::ReplayComplete => {
            println!(
                "[probe +{:>5}ms] replay complete",
                started_at.elapsed().as_millis()
            );
        }
        SubscribeMessage::Event(envelope) => {
            let event_summary = match &envelope.event {
                SessionEvent::SessionStartupChanged { state } => match state {
                    SessionStartupState::Pending => "startup pending".to_owned(),
                    SessionStartupState::Ready => "startup ready".to_owned(),
                    SessionStartupState::Running { stage, message } => {
                        format!("startup running stage={stage:?} message={message}")
                    }
                    SessionStartupState::Failed { stage, message } => {
                        format!("startup failed stage={stage:?} message={message}")
                    }
                },
                SessionEvent::AgentStateChanged { role, state } => {
                    format!("agent state role={role:?} state={state:?}")
                }
                SessionEvent::BlockAppend {
                    role,
                    block_id,
                    block,
                } => {
                    format!(
                        "block append role={role:?} block_id={} block={block:?}",
                        block_id.0
                    )
                }
                SessionEvent::BlockPatch {
                    role,
                    block_id,
                    patch,
                } => {
                    format!(
                        "block patch role={role:?} block_id={} patch={patch:?}",
                        block_id.0
                    )
                }
                SessionEvent::TaskStarted {
                    task_id,
                    title,
                    description,
                } => {
                    format!(
                        "task started id={} title={title:?} description={description:?}",
                        task_id.0
                    )
                }
                SessionEvent::TaskStatusChanged { task_id, status } => {
                    format!("task status id={} status={status:?}", task_id.0)
                }
                SessionEvent::ContextUpdated {
                    role,
                    remaining_percent,
                } => {
                    format!("context updated role={role:?} remaining={remaining_percent}%")
                }
                SessionEvent::AgentModelChanged {
                    role,
                    model_id,
                    available_models,
                } => {
                    format!(
                        "model changed role={role:?} model_id={model_id:?} available={available_models:?}"
                    )
                }
            };
            println!(
                "[probe +{:>5}ms] seq={} {}",
                started_at.elapsed().as_millis(),
                envelope.seq,
                event_summary
            );
        }
    }
}
