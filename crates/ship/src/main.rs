use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::any,
};
use eyre::Context;
use axum::body::Body;
use roam::{
    AcceptedConnection, ConnectionAcceptor, ConnectionSettings, Driver, Metadata, MetadataEntry,
    MetadataValue, NoopCaller,
};
use ship_db::ShipDb;
use ship_frontend_impl::FrontendImpl;
use ship_frontend_service::FrontendDispatcher;
use ship_policy::ParticipantName;
use ship_policy::RoomId;
use ship_runtime::Runtime;
use ship_tool_impl::ToolBackendImpl;
use ship_tool_service::ToolBackendDispatcher;
use tokio::sync::Mutex;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

type Request = axum::http::Request<Body>;

#[derive(Clone)]
struct AppState {
    runtime: Arc<Mutex<Runtime>>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("ship {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy()
        }))
        .init();

    let db = ShipDb::open_in_memory().wrap_err("failed to open database")?;
    let runtime = Arc::new(Mutex::new(Runtime::new(db)));

    let state = AppState {
        runtime: runtime.clone(),
    };

    let app = Router::new()
        .route("/ws/frontend", any(frontend_ws_handler))
        .route("/ws/tool", any(tool_ws_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .wrap_err("failed to bind")?;
    let addr = listener.local_addr()?;
    tracing::info!(%addr, "ship listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .wrap_err("server error")?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl+c handler");
    tracing::info!("shutting down");
}

// ── Frontend WebSocket ───────────────────────────────────────────────

async fn frontend_ws_handler(
    State(state): State<AppState>,
    mut request: Request,
) -> impl IntoResponse {
    if !hyper_tungstenite::is_upgrade_request(&request) {
        return (StatusCode::BAD_REQUEST, "expected websocket upgrade").into_response();
    }

    let (response, websocket) = match hyper_tungstenite::upgrade(&mut request, None) {
        Ok(ok) => ok,
        Err(error) => {
            tracing::warn!(%error, "failed to upgrade frontend websocket");
            return (StatusCode::BAD_REQUEST, "invalid websocket upgrade").into_response();
        }
    };

    let runtime = state.runtime.clone();
    tokio::spawn(async move {
        let ws_stream = match websocket.await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "frontend websocket upgrade failed");
                return;
            }
        };

        let frontend_impl = FrontendImpl::new(runtime);
        let link = roam_websocket::WsLink::new(ws_stream);
        match roam::acceptor(link)
            .establish::<NoopCaller>(FrontendDispatcher::new(frontend_impl))
            .await
        {
            Ok((_caller, _session_handle)) => {
                std::future::pending::<()>().await;
            }
            Err(error) => {
                tracing::warn!(?error, "frontend roam session failed");
            }
        }
    });

    response.map(Body::new).into_response()
}

// ── Tool Backend WebSocket ───────────────────────────────────────────

async fn tool_ws_handler(
    State(state): State<AppState>,
    mut request: Request,
) -> impl IntoResponse {
    if !hyper_tungstenite::is_upgrade_request(&request) {
        return (StatusCode::BAD_REQUEST, "expected websocket upgrade").into_response();
    }

    let (response, websocket) = match hyper_tungstenite::upgrade(&mut request, None) {
        Ok(ok) => ok,
        Err(error) => {
            tracing::warn!(%error, "failed to upgrade tool websocket");
            return (StatusCode::BAD_REQUEST, "invalid websocket upgrade").into_response();
        }
    };

    let runtime = state.runtime.clone();
    tokio::spawn(async move {
        let ws_stream = match websocket.await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "tool websocket upgrade failed");
                return;
            }
        };

        let acceptor = ToolConnectionAcceptor {
            runtime: runtime.clone(),
        };
        let link = roam_websocket::WsLink::new(ws_stream);
        match roam::acceptor(link)
            .on_connection(acceptor)
            .establish::<NoopCaller>(())
            .await
        {
            Ok((_caller, _session_handle)) => {
                std::future::pending::<()>().await;
            }
            Err(error) => {
                tracing::warn!(?error, "tool roam session failed");
            }
        }
    });

    response.map(Body::new).into_response()
}

// ── Tool Connection Acceptor ─────────────────────────────────────────

struct ToolConnectionAcceptor {
    runtime: Arc<Mutex<Runtime>>,
}

impl ConnectionAcceptor for ToolConnectionAcceptor {
    fn accept(
        &self,
        _conn_id: roam::ConnectionId,
        peer_settings: &ConnectionSettings,
        metadata: &[MetadataEntry],
    ) -> Result<AcceptedConnection, Metadata<'static>> {
        let participant_str = metadata_string(metadata, "ship-participant")
            .ok_or_else(|| rejection_metadata("missing ship-participant metadata"))?;
        let room_id_str = metadata_string(metadata, "ship-room-id")
            .ok_or_else(|| rejection_metadata("missing ship-room-id metadata"))?;

        let participant = ParticipantName::new(participant_str.to_owned());
        let room_id = RoomId::new(room_id_str.to_owned());
        let runtime = self.runtime.clone();

        let settings = ConnectionSettings {
            parity: peer_settings.parity.other(),
            max_concurrent_requests: 64,
        };

        Ok(AcceptedConnection {
            settings,
            metadata: Vec::new(),
            setup: Box::new(move |connection| {
                let tool_impl = ToolBackendImpl::new(runtime, participant, room_id);
                tokio::spawn(async move {
                    let mut driver =
                        Driver::new(connection, ToolBackendDispatcher::new(tool_impl));
                    driver.run().await;
                });
            }),
        })
    }
}

fn metadata_string<'a>(metadata: &'a [MetadataEntry], key: &str) -> Option<&'a str> {
    metadata.iter().find_map(|entry| {
        if entry.key != key {
            return None;
        }
        match entry.value {
            MetadataValue::String(value) => Some(value),
            _ => None,
        }
    })
}

fn rejection_metadata(reason: &str) -> Metadata<'static> {
    vec![MetadataEntry {
        key: "rejection-reason",
        value: MetadataValue::String(Box::leak(reason.to_owned().into_boxed_str())),
        flags: roam::MetadataFlags::NONE,
    }]
}
