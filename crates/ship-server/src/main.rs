mod ship_impl;

use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use figue::{self as args, FigueBuiltins};
use ship_core::ProjectRegistry;
use ship_impl::ShipImpl;
use ship_service::{ShipClient, ShipDispatcher};
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

    /// Manage projects.
    Project {
        /// Project command.
        #[facet(args::subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Debug, facet::Facet)]
struct ServeArgs {
    /// HTTP listen address (for example: [::]:9140).
    #[facet(args::named, default)]
    listen: Option<String>,
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
    DevProxy { vite_origin: String },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Args = args::from_std_args().into_result()?.value;

    match args.command {
        Command::Serve(args) => run_serve(args).await,
        Command::Project { command } => run_project(command).await,
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

    let listen_addr = resolve_listen_addr(args.listen)?;

    let frontend_mode = load_frontend_mode();
    // r[server.config-dir]
    let mut project_registry = ProjectRegistry::load_default().await?;
    // r[project.validation]
    project_registry.validate_all().await?;
    ensure_project_ship_gitignored(&project_registry)?;

    let sessions_dir = project_registry.config_dir().join("sessions");
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let state = AppState {
        ship: ShipImpl::new(project_registry, sessions_dir, repo_root),
        http_client: reqwest::Client::new(),
        frontend_mode,
    };

    let app = Router::new()
        // r[backend.rpc]
        .route("/ws", any(ws_handler))
        .fallback(proxy_vite_handler)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    let url = format!("http://{}", listener.local_addr()?);
    // r[cli.open-browser]
    println!("Ship server listening at {url}");
    tracing::info!(%url, "ship server started");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
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
fn resolve_listen_addr(
    cli_listen: Option<String>,
) -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let listen = cli_listen
        .or_else(|| std::env::var("SHIP_LISTEN").ok())
        .unwrap_or_else(|| "[::]:9140".to_owned());
    Ok(listen.parse::<SocketAddr>()?)
}

// r[server.mode]
fn load_frontend_mode() -> FrontendMode {
    let vite_origin =
        std::env::var("SHIP_VITE_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:5173".to_owned());
    FrontendMode::DevProxy { vite_origin }
}

// r[backend.persistence-dir-gitignore]
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
        let ws_stream = match websocket.await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "websocket upgrade future failed");
                return;
            }
        };

        let link = roam_websocket::WsLink::new(ws_stream);
        match roam::acceptor(link)
            .establish::<ShipClient>(ShipDispatcher::new(ship))
            .await
        {
            Ok((caller_guard, _session_handle)) => {
                let _caller_guard = caller_guard;
                std::future::pending::<()>().await;
            }
            Err(error) => {
                tracing::warn!(?error, "failed to establish roam websocket session");
            }
        }
    });

    response.map(Body::new).into_response()
}

async fn proxy_vite_handler(
    State(state): State<AppState>,
    request: Request,
) -> axum::response::Response {
    let FrontendMode::DevProxy { vite_origin } = &state.frontend_mode;

    let (parts, body) = request.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let target_url = format!("{vite_origin}{path_and_query}");

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
