mod agent_discovery;
mod captain_mcp;
mod ship_impl;
mod transcriber;

use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_discovery::{SystemBinaryPathProbe, discover_agents};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use figue::{self as args, FigueBuiltins};
use futures_util::{SinkExt, StreamExt};
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
struct AppState {
    ship: ShipImpl,
    http_client: reqwest::Client,
    frontend_mode: FrontendMode,
}

#[derive(Debug, facet::Facet)]
// r[cli.binary]
struct Args {
    /// Subcommand to run.
    #[facet(args::subcommand)]
    command: Command,

    /// Standard CLI builtins (--help, --version, --completions).
    #[facet(flatten)]
    builtins: FigueBuiltins,
}

#[derive(Debug, facet::Facet)]
#[repr(u8)]
enum Command {
    /// Start the Ship server.
    Serve(ServeArgs),

    /// Run the captain MCP stdio server.
    CaptainMcpServer(McpServerArgs),

    /// Run the mate MCP stdio server.
    MateMcpServer(McpServerArgs),

    /// Manage projects.
    Project {
        /// Project command.
        #[facet(args::subcommand)]
        command: ProjectCommand,
    },

    /// Spin up an in-process server, create a session, and watch events.
    Probe(ProbeArgs),

    /// Listen to the default mic and transcribe speech using Silero VAD + Whisper.
    Listen(ListenArgs),
}

#[derive(Debug, facet::Facet)]
struct ProbeArgs {
    /// Project name.
    #[facet(args::positional)]
    project: String,

    /// Base branch for the session.
    #[facet(args::named, default)]
    base_branch: Option<String>,

    /// Captain agent kind (claude or codex).
    #[facet(args::named, default)]
    captain: Option<AgentKind>,

    /// Mate agent kind (claude or codex).
    #[facet(args::named, default)]
    mate: Option<AgentKind>,

    /// Delay in milliseconds before sending the captain prompt.
    #[facet(args::named, default)]
    prompt_after_ms: Option<u64>,

    /// Prompt to send to the captain after the delay.
    #[facet(args::named, default)]
    prompt: Option<String>,

    /// Idle timeout in milliseconds (exit when no events arrive for this long).
    #[facet(args::named, default)]
    idle_timeout_ms: Option<u64>,
}

#[derive(Debug, facet::Facet)]
struct ServeArgs {
    /// HTTP listen address (for example: `[::1]:9140`).
    #[facet(args::named, default)]
    listen: Option<String>,
}

#[derive(Debug, facet::Facet)]
struct McpServerArgs {
    /// Session id.
    #[facet(args::named)]
    session: String,

    /// Ship server websocket URL.
    #[facet(args::named)]
    server_ws_url: String,
}

#[derive(Debug, facet::Facet)]
#[repr(u8)]
enum ProjectCommand {
    /// Register a project.
    Add(ProjectAddArgs),

    /// List registered projects.
    List,

    /// Remove a registered project.
    Remove(ProjectRemoveArgs),
}

#[derive(Debug, facet::Facet)]
struct ListenArgs {
    /// Path to whisper model (overrides SHIP_WHISPER_MODEL).
    #[facet(args::named, default)]
    whisper_model: Option<String>,
}

#[derive(Debug, facet::Facet)]
struct ProjectAddArgs {
    /// Path to repository.
    #[facet(args::positional)]
    path: String,
}

#[derive(Debug, facet::Facet)]
struct ProjectRemoveArgs {
    /// Project name.
    #[facet(args::positional)]
    name: String,
}

#[derive(Clone)]
enum FrontendMode {
    // vite_origin: loopback URL used by Axum to proxy HTTP requests to Vite
    // vite_port: Vite's port, used for WebSocket proxy (HMR)
    DevProxy { vite_origin: String, vite_port: u16 },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Args = args::from_std_args().unwrap();

    match args.command {
        Command::Serve(args) => run_serve(args).await,
        Command::CaptainMcpServer(args) => {
            captain_mcp::run_captain_stdio_server(captain_mcp::CaptainMcpServerArgs {
                session_id: SessionId(args.session),
                server_ws_url: args.server_ws_url,
            })
            .await?;
            Ok(())
        }
        Command::MateMcpServer(args) => {
            captain_mcp::run_mate_stdio_server(captain_mcp::MateMcpServerArgs {
                session_id: SessionId(args.session),
                server_ws_url: args.server_ws_url,
            })
            .await?;
            Ok(())
        }
        Command::Project { command } => run_project(command).await,
        Command::Probe(args) => run_probe(args).await,
        Command::Listen(args) => run_listen(args).await,
    }
}

#[derive(Clone)]
struct ProbeState {
    ship: ShipImpl,
}

async fn probe_ws_handler(
    State(state): State<ProbeState>,
    mut request: Request,
) -> impl IntoResponse {
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

async fn run_probe(args: ProbeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let base_branch = args.base_branch.unwrap_or_else(|| "main".to_owned());
    let captain_kind = args.captain.unwrap_or(AgentKind::Claude);
    let mate_kind = args.mate.unwrap_or(AgentKind::Claude);
    let idle_timeout_ms = args.idle_timeout_ms.unwrap_or(10_000);

    if args.prompt_after_ms.is_some() && args.prompt.is_none() {
        return Err("--prompt-after-ms requires --prompt".into());
    }

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
        .route("/ws", any(probe_ws_handler))
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
            captain_kind,
            mate_kind,
            base_branch: base_branch.clone(),
            mcp_servers: None,
        })
        .await
        .map_err(|error| format!("create_session failed: {error:?}"))?;

    let session_id = match response {
        CreateSessionResponse::Created { session_id, .. } => session_id,
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
        match timeout(Duration::from_millis(idle_timeout_ms), rx.recv()).await {
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
                    idle_timeout_ms
                );
                break;
            }
        }
    }

    server_task.abort();
    Ok(())
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
                SessionEvent::AgentEffortChanged {
                    role,
                    effort_config_id,
                    effort_value_id,
                    ..
                } => {
                    format!(
                        "effort changed role={role:?} config_id={effort_config_id:?} value_id={effort_value_id:?}"
                    )
                }
                SessionEvent::MateGuidanceQueued { .. } => "mate guidance queued".to_owned(),
                SessionEvent::HumanReviewRequested { .. } => "human review requested".to_owned(),
                SessionEvent::HumanReviewCleared => "human review cleared".to_owned(),
                SessionEvent::SessionTitleChanged { title } => {
                    format!("session title changed: {title}")
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

// r[cli.serve]
async fn run_serve(args: ServeArgs) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy()
        }))
        .init();

    let listen_addrs = resolve_listen_addrs(args.listen)?;
    let primary_addr = listen_addrs
        .iter()
        .find(|a| a.ip().is_loopback())
        .copied()
        .unwrap_or(listen_addrs[0]);
    // Use the first non-loopback address for Vite HMR so mobile devices on the
    // LAN can reach the HMR WebSocket. Falls back to loopback (localhost-only).
    let hmr_addr = listen_addrs
        .iter()
        .find(|a| !a.ip().is_loopback())
        .copied()
        .unwrap_or(primary_addr);
    let vite_addr = resolve_vite_addr()?;
    // r[dev-proxy.vite-lifecycle]
    let _vite_process = spawn_vite_dev_server(hmr_addr, vite_addr).await?;
    wait_for_tcp_readiness(vite_addr, Duration::from_secs(10)).await?;

    let frontend_mode = load_frontend_mode(vite_addr);
    let agent_discovery = discover_agents(&SystemBinaryPathProbe);
    // r[server.config-dir]
    let mut project_registry = ProjectRegistry::load_default().await?;
    // r[project.validation]
    project_registry.validate_all().await?;
    ensure_project_ship_gitignored(&project_registry)?;

    let sessions_dir = project_registry.config_dir().join("sessions");
    let ship = ShipImpl::new(project_registry, sessions_dir, agent_discovery);
    // r[resilience.server-restart]
    ship.load_persisted_sessions().await;
    ship.fetch_github_user_avatar().await;
    ship.configure_whisper_model();
    let state = AppState {
        ship: ship.clone(),
        http_client: reqwest::Client::new(),
        frontend_mode,
    };

    let app = Router::new()
        // r[backend.rpc]
        .route("/ws", any(ws_handler))
        .fallback(proxy_vite_handler)
        .with_state(state);

    // Bind a listener on every resolved address.
    let mut listeners: Vec<tokio::net::TcpListener> = Vec::new();
    for addr in &listen_addrs {
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => {
                tracing::debug!(addr = %l.local_addr()?, "bound listener");
                listeners.push(l);
            }
            Err(e) => {
                tracing::warn!(%addr, "failed to bind: {e}");
            }
        }
    }
    if listeners.is_empty() {
        return Err("failed to bind on any address".into());
    }

    // Use the loopback listener for the agent WS URL.
    let ws_listener = listeners
        .iter()
        .find(|l| {
            l.local_addr()
                .map(|a| a.ip().is_loopback())
                .unwrap_or(false)
        })
        .unwrap_or(&listeners[0]);
    ship.set_server_ws_url(format!("ws://{}/ws", ws_listener.local_addr()?));

    // r[cli.open-browser]
    for l in &listeners {
        let url = format!("http://{}", l.local_addr()?);
        println!("Ship server listening at {url}");
        tracing::info!(%url, "ship server started");
    }

    let mut http_urls: Vec<String> = listeners
        .iter()
        .filter(|l| !l.local_addr().map(|a| a.ip().is_loopback()).unwrap_or(true))
        .map(|l| format!("http://{}", l.local_addr().unwrap()))
        .collect();
    http_urls.extend(
        listeners
            .iter()
            .filter(|l| {
                l.local_addr()
                    .map(|a| a.ip().is_loopback())
                    .unwrap_or(false)
            })
            .map(|l| format!("http://{}", l.local_addr().unwrap())),
    );
    ship.set_listen_http_urls(http_urls);

    // Shared shutdown signal broadcast via Notify.
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let shutdown_driver = shutdown.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_driver.notify_waiters();
    });

    let mut join_set = tokio::task::JoinSet::new();
    for listener in listeners {
        let app = app.clone();
        let shutdown = shutdown.clone();
        join_set.spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move { shutdown.notified().await })
                .await
        });
    }
    while let Some(res) = join_set.join_next().await {
        res??;
    }
    Ok(())
}

// r[cli.project-add]
async fn project_add(path: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = ProjectRegistry::load_default().await?;
    match registry.add(&path).await {
        Ok(project) => {
            println!("added project '{}' at {}", project.name.0, project.path);
            Ok(())
        }
        Err(error) => {
            eprintln!("failed to add project '{}': {error}", path);
            Err(error.into())
        }
    }
}

// r[cli.project-list]
async fn project_list() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ProjectRegistry::load_default().await?;
    for project in registry.list() {
        if project.valid {
            println!("{}\t{}\tvalid", project.name.0, project.path);
        } else {
            let reason = project
                .invalid_reason
                .as_deref()
                .unwrap_or("unknown validation error");
            println!("{}\t{}\tinvalid: {}", project.name.0, project.path, reason);
        }
    }
    Ok(())
}

// r[cli.project-remove]
async fn project_remove(name: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = ProjectRegistry::load_default().await?;
    let removed = registry.remove(&name).await?;
    if removed {
        println!("removed project '{}'", name);
    } else {
        println!("project '{}' not found", name);
    }
    Ok(())
}

async fn run_project(command: ProjectCommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ProjectCommand::Add(args) => project_add(args.path).await,
        ProjectCommand::List => project_list().await,
        ProjectCommand::Remove(args) => project_remove(args.name).await,
    }
}

// r[server.listen]
fn resolve_listen_addrs(
    cli_listen: Option<String>,
) -> Result<Vec<SocketAddr>, Box<dyn std::error::Error>> {
    // Explicit address from CLI or env → single bind, old behavior.
    if let Some(addr) = cli_listen.or_else(|| std::env::var("SHIP_LISTEN").ok()) {
        return Ok(vec![addr.parse::<SocketAddr>()?]);
    }

    Ok(vec!["127.0.0.1:9140".parse()?])
}

// r[dev-proxy.vite-port]
fn resolve_vite_addr() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let vite_addr = std::env::var("SHIP_VITE_ADDR").unwrap_or_else(|_| "127.0.0.1:5173".to_owned());
    Ok(vite_addr.parse::<SocketAddr>()?)
}

// r[server.mode]
fn load_frontend_mode(vite_addr: SocketAddr) -> FrontendMode {
    let vite_origin = format!("http://{vite_addr}");
    FrontendMode::DevProxy {
        vite_origin,
        vite_port: vite_addr.port(),
    }
}

async fn spawn_vite_dev_server(
    hmr_addr: SocketAddr,
    vite_addr: SocketAddr,
) -> Result<tokio::process::Child, Box<dyn std::error::Error>> {
    let frontend_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../frontend");
    // When there's a LAN address, bind Vite on all interfaces so mobile HMR works.
    // Axum still connects to Vite via loopback (vite_addr).
    let vite_bind_host = if hmr_addr.ip().is_loopback() {
        vite_addr.ip().to_string()
    } else {
        "0.0.0.0".to_owned()
    };
    let mut child = tokio::process::Command::new("pnpm");
    child
        .arg("exec")
        .arg("vite")
        .arg("--clearScreen")
        .arg("false")
        .arg("--strictPort")
        .arg("--host")
        .arg(vite_bind_host)
        .arg("--port")
        .arg(vite_addr.port().to_string())
        .env("SHIP_VITE_HMR_HOST", vite_hmr_host(hmr_addr))
        .env("SHIP_VITE_HMR_CLIENT_PORT", hmr_addr.port().to_string())
        .current_dir(frontend_dir)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    Ok(child.spawn()?)
}

fn vite_hmr_host(listen_addr: SocketAddr) -> String {
    match listen_addr.ip() {
        std::net::IpAddr::V4(ip) if ip.is_unspecified() || ip.is_loopback() => {
            "localhost".to_owned()
        }
        std::net::IpAddr::V6(ip) if ip.is_unspecified() || ip.is_loopback() => {
            "localhost".to_owned()
        }
        ip => ip.to_string(),
    }
}

async fn wait_for_tcp_readiness(
    vite_addr: SocketAddr,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::net::TcpStream::connect(vite_addr).await {
            Ok(stream) => {
                drop(stream);
                return Ok(());
            }
            Err(error) => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(
                        format!("timed out waiting for Vite at {vite_addr}: {error}").into(),
                    );
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

// r[backend.persistence-dir-gitignore]
// r[worktree.gitignore]
fn ensure_project_ship_gitignored(
    registry: &ProjectRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    for project in registry.list().into_iter().filter(|project| project.valid) {
        ensure_ship_entry_for_project(Path::new(&project.path))?;
    }
    Ok(())
}

fn ensure_ship_entry_for_project(project_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let gitignore_path = project_path.join(".gitignore");
    let existing = match std::fs::read_to_string(&gitignore_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };

    let has_entry = existing.lines().any(|line| line.trim() == ".ship/");
    if has_entry {
        return Ok(());
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore_path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(b".ship/\n")?;
    Ok(())
}

async fn ws_handler(State(state): State<AppState>, mut request: Request) -> impl IntoResponse {
    tracing::info!("ws_handler entered");
    if !hyper_tungstenite::is_upgrade_request(&request) {
        return (StatusCode::BAD_REQUEST, "expected websocket upgrade").into_response();
    }

    let (response, websocket) = match hyper_tungstenite::upgrade(&mut request, None) {
        Ok(ok) => ok,
        Err(error) => {
            tracing::warn!(%error, "failed to upgrade websocket request");
            return (StatusCode::BAD_REQUEST, "invalid websocket upgrade").into_response();
        }
    };

    let ship = state.ship.clone();
    tokio::spawn(async move {
        tracing::info!("accepting websocket upgrade");
        let ws_stream = match websocket.await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "websocket upgrade future failed");
                return;
            }
        };
        tracing::info!("websocket upgrade complete");

        let link = roam_websocket::WsLink::new(ws_stream);
        match roam::acceptor(link)
            .on_connection(ship.ship_mcp_connection_acceptor())
            .establish::<ShipClient>(ShipDispatcher::new(ship))
            .await
        {
            Ok((caller_guard, _session_handle)) => {
                let _caller_guard = caller_guard;
                tracing::info!("roam websocket session established");
                std::future::pending::<()>().await;
            }
            Err(error) => {
                tracing::warn!(?error, "roam websocket session closed or failed");
            }
        }
    });

    response.map(Body::new).into_response()
}

fn is_websocket_upgrade(req: &Request) -> bool {
    req.headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
}

async fn proxy_vite_handler(
    State(state): State<AppState>,
    request: Request,
) -> axum::response::Response {
    let FrontendMode::DevProxy {
        vite_origin,
        vite_port,
    } = &state.frontend_mode;

    let path = request.uri().path().to_string();
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();

    // WebSocket upgrade (Vite HMR) — proxy as WebSocket
    if is_websocket_upgrade(&request) {
        tracing::debug!(path = %path, "detected websocket upgrade for vite HMR");
        return handle_vite_ws_upgrade(request, *vite_port, path, query).await;
    }

    // Regular HTTP proxy
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let target_url = format!("{vite_origin}{path_and_query}");

    let (parts, body) = request.into_parts();

    let body = match to_bytes(body, 8 * 1024 * 1024).await {
        Ok(body) => body,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read request body: {error}"),
            )
                .into_response();
        }
    };

    let mut upstream = state.http_client.request(
        reqwest::Method::from_str(parts.method.as_str()).unwrap_or(reqwest::Method::GET),
        target_url,
    );

    for (name, value) in &parts.headers {
        if should_skip_request_header(name) {
            continue;
        }
        upstream = upstream.header(name, value);
    }

    if !body.is_empty() {
        upstream = upstream.body(body.to_vec());
    }

    let upstream_response = match upstream.send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("Vite proxy request failed: {error}"),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(upstream_response.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    let response_headers = upstream_response.headers().clone();
    let response_body = match upstream_response.bytes().await {
        Ok(body) => body,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("failed to read Vite response body: {error}"),
            )
                .into_response();
        }
    };

    let mut response = Response::new(Body::from(response_body));
    *response.status_mut() = status;
    for (name, value) in &response_headers {
        if should_skip_response_header(name) {
            continue;
        }
        response.headers_mut().append(name.clone(), value.clone());
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Handle WebSocket upgrade for Vite HMR proxy
async fn handle_vite_ws_upgrade(
    mut request: Request,
    vite_port: u16,
    path: String,
    query: String,
) -> axum::response::Response {
    if let Some(protocol) = request.headers().get("sec-websocket-protocol") {
        tracing::debug!(protocol = ?protocol, "incoming vite websocket protocol header");
    }

    if !hyper_tungstenite::is_upgrade_request(&request) {
        return (StatusCode::BAD_REQUEST, "expected websocket upgrade").into_response();
    }

    let (mut response, websocket) = match hyper_tungstenite::upgrade(&mut request, None) {
        Ok(ok) => ok,
        Err(e) => {
            tracing::warn!(error = %e, "failed to upgrade vite websocket");
            return (
                StatusCode::BAD_REQUEST,
                format!("WebSocket upgrade failed: {e}"),
            )
                .into_response();
        }
    };

    // Echo back the Sec-WebSocket-Protocol header so the browser accepts the upgrade.
    if let Some(protocol) = request.headers().get("sec-websocket-protocol") {
        response
            .headers_mut()
            .insert("sec-websocket-protocol", protocol.clone());
    }

    tokio::spawn(async move {
        match websocket.await {
            Ok(client_ws) => {
                tracing::debug!(path = %path, "vite HMR websocket established, starting proxy");
                if let Err(e) = proxy_vite_ws(client_ws, vite_port, &path, &query).await {
                    tracing::warn!(error = %e, path = %path, "vite websocket proxy error");
                }
                tracing::debug!(path = %path, "vite HMR websocket closed");
            }
            Err(e) => {
                tracing::warn!(error = %e, "vite HMR websocket upgrade failed");
            }
        }
    });

    response.into_response()
}

/// Bidirectional WebSocket proxy to Vite dev server
async fn proxy_vite_ws<S>(
    client_socket: S,
    vite_port: u16,
    path: &str,
    query: &str,
) -> eyre::Result<()>
where
    S: futures_util::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + futures_util::Sink<tokio_tungstenite::tungstenite::Message>
        + Unpin
        + Send,
{
    use tokio_tungstenite::connect_async_with_config;
    use tokio_tungstenite::tungstenite;

    // Try both IPv6 and IPv4
    let addrs: &[&str] = &["[::1]", "127.0.0.1"];
    let mut last_error: Option<eyre::Report> = None;

    for addr in addrs {
        let vite_url = format!("ws://{addr}:{vite_port}{path}{query}");
        tracing::trace!(vite_url = %vite_url, "attempting vite websocket connection");

        let request = tungstenite::http::Request::builder()
            .uri(&vite_url)
            .header("Sec-WebSocket-Protocol", "vite-hmr")
            .header("Host", format!("{addr}:{vite_port}"))
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(())?;

        let connect_result = tokio::time::timeout(
            Duration::from_secs(5),
            connect_async_with_config(request, None, false),
        )
        .await;

        match connect_result {
            Ok(Ok((vite_ws, _response))) => {
                tracing::debug!(addr = %addr, "vite websocket connected");
                return run_ws_proxy(client_socket, vite_ws).await;
            }
            Ok(Err(e)) => {
                tracing::trace!(addr = %addr, error = %e, "vite ws connect failed");
                last_error = Some(e.into());
            }
            Err(_) => {
                tracing::trace!(addr = %addr, "vite ws connect timed out");
                last_error = Some(eyre::eyre!("vite ws connect timed out"));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| eyre::eyre!("no addresses to try")))
}

/// Run bidirectional message relay between client and Vite WebSocket
async fn run_ws_proxy<C, V>(client_socket: C, vite_ws: V) -> eyre::Result<()>
where
    C: futures_util::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + futures_util::Sink<tokio_tungstenite::tungstenite::Message>
        + Unpin,
    V: futures_util::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + futures_util::Sink<tokio_tungstenite::tungstenite::Message>
        + Unpin,
{
    use tokio_tungstenite::tungstenite::Message as TungMsg;

    let (mut client_tx, mut client_rx) = client_socket.split();
    let (mut vite_tx, mut vite_rx) = vite_ws.split();

    let client_to_vite = async {
        while let Some(Ok(msg)) = client_rx.next().await {
            match &msg {
                TungMsg::Text(_) | TungMsg::Binary(_) => {
                    if vite_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                TungMsg::Close(_) => break,
                _ => {}
            }
        }
    };

    let vite_to_client = async {
        while let Some(Ok(msg)) = vite_rx.next().await {
            match &msg {
                TungMsg::Text(_) | TungMsg::Binary(_) => {
                    if client_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                TungMsg::Close(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = client_to_vite => {}
        _ = vite_to_client => {}
    }

    Ok(())
}

fn should_skip_request_header(name: &HeaderName) -> bool {
    let lower = name.as_str().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "connection"
            | "upgrade"
            | "host"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "content-length"
    )
}

fn should_skip_response_header(name: &HeaderName) -> bool {
    let lower = name.as_str().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "connection"
            | "upgrade"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "content-length"
            | "cache-control"
            | "etag"
            | "last-modified"
    )
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        signal.recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

async fn run_listen(args: ListenArgs) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
        .init();

    // Resolve whisper model path
    let whisper_model = args
        .whisper_model
        .or_else(|| std::env::var("SHIP_WHISPER_MODEL").ok())
        .ok_or("whisper model required: pass --whisper-model or set SHIP_WHISPER_MODEL")?;

    tracing::info!(whisper = %whisper_model, "loading models");

    let mut transcriber = transcriber::SpeechTranscriber::new(Path::new(&whisper_model))?;

    // Set up mic capture via cpal
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("no default input device")?;
    tracing::info!(
        device = device.name().unwrap_or_default(),
        "using input device"
    );

    // Use the device's default config — the mic may not support 1ch/16kHz directly.
    let default_config = device.default_input_config()?;
    let native_sample_rate = default_config.sample_rate().0;
    let native_channels = default_config.channels();
    tracing::info!(
        sample_rate = native_sample_rate,
        channels = native_channels,
        "native input config"
    );

    let config = cpal::StreamConfig {
        channels: native_channels,
        sample_rate: default_config.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    let (audio_tx, mut audio_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<f32>>();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            // Downmix to mono if needed
            let mono: Vec<f32> = if native_channels == 1 {
                data.to_vec()
            } else {
                data.chunks_exact(native_channels as usize)
                    .map(|frame| frame.iter().sum::<f32>() / native_channels as f32)
                    .collect()
            };

            // Resample to 16kHz if needed
            if native_sample_rate == 16000 {
                let _ = audio_tx.send(mono);
            } else {
                let ratio = 16000.0 / native_sample_rate as f64;
                let out_len = (mono.len() as f64 * ratio) as usize;
                let mut resampled = Vec::with_capacity(out_len);
                for i in 0..out_len {
                    let src_idx = i as f64 / ratio;
                    let idx = src_idx as usize;
                    let frac = src_idx - idx as f64;
                    let sample = if idx + 1 < mono.len() {
                        mono[idx] as f64 * (1.0 - frac) + mono[idx + 1] as f64 * frac
                    } else if idx < mono.len() {
                        mono[idx] as f64
                    } else {
                        0.0
                    };
                    resampled.push(sample as f32);
                }
                let _ = audio_tx.send(resampled);
            }
        },
        |err| {
            tracing::error!("audio input error: {err}");
        },
        None,
    )?;
    stream.play()?;
    tracing::info!("listening... press Ctrl-C to stop");

    let mut transcribed_segments: Vec<String> = Vec::new();

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                if let Some(seg) = transcriber.flush() {
                    eprintln!();
                    tracing::info!("transcribed (final): {}", seg.text);
                    transcribed_segments.push(seg.text);
                }
                tracing::info!("stopping");
                break;
            }
            Some(samples) = audio_rx.recv() => {
                for event in transcriber.feed(&samples) {
                    match event {
                        transcriber::SpeechEvent::SpeechStarted { sample } => {
                            eprint!(
                                "\r\x1b[2K[speech started at {:.1}s]",
                                sample as f64 / 16000.0
                            );
                            let _ = std::io::stderr().flush();
                        }
                        transcriber::SpeechEvent::SpeechEnded { segment } => {
                            eprintln!();
                            tracing::info!("transcribed: {}", segment.text);
                            transcribed_segments.push(segment.text);
                        }
                        transcriber::SpeechEvent::None => {
                            let state = if transcriber.is_speaking() {
                                format!("speaking ({:.1}s)", transcriber.speech_duration_secs())
                            } else {
                                "silence".to_owned()
                            };
                            eprint!(
                                "\r\x1b[2K[{:.1}s] [{}] [segments: {}]",
                                transcriber.total_samples() as f64 / 16000.0,
                                state,
                                transcribed_segments.len(),
                            );
                            let _ = std::io::stderr().flush();
                        }
                        transcriber::SpeechEvent::Error(e) => {
                            tracing::warn!("transcriber error: {e}");
                        }
                    }
                }
            }
        }
    }

    // Print final result
    if !transcribed_segments.is_empty() {
        println!("\n--- Final transcription ---");
        println!("{}", transcribed_segments.join(" "));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::{LazyLock, Mutex, MutexGuard};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ensure_ship_entry_for_project, resolve_listen_addrs};

    static SHIP_LISTEN_ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn make_temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ship-server-{test_name}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    struct ShipListenEnvGuard {
        _lock: MutexGuard<'static, ()>,
        original: Option<String>,
    }

    impl ShipListenEnvGuard {
        fn set(value: Option<&str>) -> Self {
            let lock = SHIP_LISTEN_ENV_LOCK
                .lock()
                .expect("SHIP_LISTEN test lock should not be poisoned");
            let original = std::env::var("SHIP_LISTEN").ok();
            match value {
                Some(value) => unsafe { std::env::set_var("SHIP_LISTEN", value) },
                None => unsafe { std::env::remove_var("SHIP_LISTEN") },
            }
            Self {
                _lock: lock,
                original,
            }
        }
    }

    impl Drop for ShipListenEnvGuard {
        fn drop(&mut self) {
            match self.original.as_deref() {
                Some(value) => unsafe { std::env::set_var("SHIP_LISTEN", value) },
                None => unsafe { std::env::remove_var("SHIP_LISTEN") },
            }
        }
    }

    // r[verify backend.persistence-dir-gitignore]
    // r[verify worktree.gitignore]
    #[test]
    fn ensure_ship_entry_appends_ship_root_once() {
        let dir = make_temp_dir("gitignore-entry");
        let gitignore = dir.join(".gitignore");

        std::fs::write(&gitignore, "target/\n").expect("gitignore should be written");
        ensure_ship_entry_for_project(&dir).expect("ship entry should be added");
        ensure_ship_entry_for_project(&dir).expect("ship entry should not duplicate");

        let contents = std::fs::read_to_string(&gitignore).expect("gitignore should be readable");
        assert_eq!(contents, "target/\n.ship/\n");

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify server.listen]
    #[test]
    fn resolve_listen_addr_defaults_to_loopback() {
        let _env_guard = ShipListenEnvGuard::set(None);
        let addrs = resolve_listen_addrs(None).expect("default listen addresses should parse");
        assert!(
            addrs.iter().any(|a| a.ip().is_loopback()),
            "expected at least one loopback address, got: {addrs:?}"
        );
    }

    // r[verify server.listen]
    #[test]
    fn resolve_listen_addr_uses_ship_listen_env_before_default() {
        let _env_guard = ShipListenEnvGuard::set(Some("127.0.0.1:9200"));
        let addrs = resolve_listen_addrs(None).expect("env listen address should parse");
        assert_eq!(addrs, vec!["127.0.0.1:9200".parse::<SocketAddr>().unwrap()]);
    }

    // r[verify server.listen]
    #[test]
    fn resolve_listen_addr_prefers_cli_over_ship_listen_env() {
        let _env_guard = ShipListenEnvGuard::set(Some("127.0.0.1:9200"));
        let addrs = resolve_listen_addrs(Some("[::1]:9300".to_owned()))
            .expect("cli listen address should parse");
        assert_eq!(addrs, vec!["[::1]:9300".parse::<SocketAddr>().unwrap()]);
    }
}
