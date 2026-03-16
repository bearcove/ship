use std::fmt;

use roam::{ConnectionSettings, MetadataEntry, MetadataFlags, MetadataValue, NoopCaller, Parity};

#[derive(Debug)]
pub enum ConnectError {
    WebSocket(String),
    Roam(String),
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WebSocket(msg) => write!(f, "websocket connection failed: {msg}"),
            Self::Roam(msg) => write!(f, "roam session failed: {msg}"),
        }
    }
}

impl std::error::Error for ConnectError {}

/// Connect to a ship server over websocket and establish a roam session.
///
/// Returns the roam caller and a background driver task handle.
/// The caller can be converted into a typed client (e.g. `AdmiralMcpClient::from(caller)`).
pub async fn connect_to_ship(
    ws_url: &str,
    service_name: &str,
    session_id: &str,
) -> Result<(roam::Caller, roam::RootGuard, tokio::task::JoinHandle<()>), ConnectError> {
    let ws_stream = tokio_tungstenite::connect_async(ws_url)
        .await
        .map_err(|e| ConnectError::WebSocket(e.to_string()))?
        .0;

    let link = roam_websocket::WsLink::new(ws_stream);
    let (_root_guard, session_handle) = roam::initiator(link)
        .establish::<NoopCaller>(())
        .await
        .map_err(|e| ConnectError::Roam(format!("{e:?}")))?;

    let connection = session_handle
        .open_connection(
            ConnectionSettings {
                parity: Parity::Odd,
                max_concurrent_requests: 64,
            },
            vec![
                MetadataEntry {
                    key: "ship-service",
                    value: MetadataValue::String(service_name),
                    flags: MetadataFlags::NONE,
                },
                MetadataEntry {
                    key: "ship-session-id",
                    value: MetadataValue::String(Box::leak(session_id.to_owned().into_boxed_str())),
                    flags: MetadataFlags::NONE,
                },
            ],
        )
        .await
        .map_err(|e| ConnectError::Roam(format!("{e:?}")))?;

    let mut driver = roam::Driver::new(connection, ());
    let caller = driver.caller();
    let driver_task = tokio::spawn(async move {
        driver.run().await;
    });

    Ok((caller, _root_guard, driver_task))
}
