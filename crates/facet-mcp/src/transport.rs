use std::fmt;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::protocol::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};

#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    InvalidJson(String),
    SerializeFailed(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "transport I/O error: {e}"),
            Self::InvalidJson(msg) => write!(f, "invalid JSON from client: {msg}"),
            Self::SerializeFailed(msg) => write!(f, "failed to serialize response: {msg}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Newline-delimited JSON-RPC over stdin/stdout.
pub struct StdioTransport {
    reader: BufReader<tokio::io::Stdin>,
    writer: tokio::io::Stdout,
}

impl StdioTransport {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(tokio::io::stdin()),
            writer: tokio::io::stdout(),
        }
    }

    pub async fn read_request(&mut self) -> Result<Option<JsonRpcRequest>, TransportError> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await.map_err(TransportError::Io)?;
        if n == 0 {
            return Ok(None);
        }
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }
        let request: JsonRpcRequest =
            facet_json::from_str(line).map_err(|e| TransportError::InvalidJson(e.to_string()))?;
        Ok(Some(request))
    }

    pub async fn write_response(
        &mut self,
        id: JsonRpcId,
        result: String,
    ) -> Result<(), TransportError> {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id: Some(id),
            result: Some(result),
            error: None,
        };
        let json = facet_json::to_string(&response)
            .map_err(|e| TransportError::SerializeFailed(e.to_string()))?;
        self.writer.write_all(json.as_bytes()).await.map_err(TransportError::Io)?;
        self.writer.write_all(b"\n").await.map_err(TransportError::Io)?;
        self.writer.flush().await.map_err(TransportError::Io)?;
        Ok(())
    }
}
