mod ship_impl;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use ship_core::ProjectRegistry;
use ship_impl::ShipImpl;
use ship_service::{ShipClient, ShipDispatcher};
use tower_http::services::{ServeDir, ServeFile};
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    ship: ShipImpl,
    http_client: reqwest::Client,
    frontend_mode: FrontendMode,
}

#[derive(Clone)]
enum FrontendMode {
    DevProxy { vite_origin: String },
    Static { dist_dir: PathBuf },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy()
        }))
        .init();

    // r[server.listen]
    let listen_addr = std::env::var("SHIP_LISTEN")
        .unwrap_or_else(|_| "[::]:9140".to_owned())
        .parse::<SocketAddr>()?;

    let frontend_mode = load_frontend_mode();
    // r[server.config-dir]
    let mut project_registry = ProjectRegistry::load_default().await?;
    // r[project.validation]
    project_registry.validate_all().await?;
    let sessions_dir = project_registry.config_dir().join("sessions");
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let state = AppState {
        ship: ShipImpl::new(project_registry, sessions_dir, repo_root),
        http_client: reqwest::Client::new(),
        frontend_mode: frontend_mode.clone(),
    };

    let app = Router::new()
        // r[backend.rpc]
        .route("/ws", any(ws_handler));

    let app = match frontend_mode {
        FrontendMode::DevProxy { .. } => app.fallback(proxy_vite_handler),
        FrontendMode::Static { dist_dir } => {
            let spa_fallback = ServeFile::new(dist_dir.join("index.html"));
            let static_service = ServeDir::new(dist_dir).not_found_service(spa_fallback);
            app.fallback_service(static_service)
        }
    }
    .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    tracing::info!("ship-server listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn load_frontend_mode() -> FrontendMode {
    if env_flag("SHIP_DEV", true) {
        let vite_origin = std::env::var("SHIP_VITE_ORIGIN")
            .unwrap_or_else(|_| "http://127.0.0.1:5173".to_owned());
        FrontendMode::DevProxy { vite_origin }
    } else {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dist_dir = std::env::var("SHIP_FRONTEND_DIST")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("frontend/dist"));
        FrontendMode::Static { dist_dir }
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
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
    let FrontendMode::DevProxy { vite_origin } = &state.frontend_mode else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };

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
