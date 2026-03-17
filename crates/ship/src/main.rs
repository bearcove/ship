use std::sync::Arc;

use eyre::Context;
use ship::AppState;
use ship_db::ShipDb;
use ship_runtime::Runtime;
use tokio::sync::Mutex;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

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

    let app = ship::router(state);

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
