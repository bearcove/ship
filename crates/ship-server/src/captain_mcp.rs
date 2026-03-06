use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{
    self, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::net::{UnixListener, UnixStream};

pub type ToolHandler =
    Arc<dyn Fn(String, Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> + Send + Sync>;

#[derive(Clone)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub struct ToolResult {
    pub text: String,
    pub is_error: bool,
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Deserialize)]
struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    protocol_version: Option<String>,
}

#[derive(Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

pub async fn serve(listener: UnixListener, tools: Vec<ToolDefinition>, handler: ToolHandler) {
    let tools = Arc::new(tools);
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tools = tools.clone();
                let handler = handler.clone();
                tokio::spawn(async move {
                    if let Err(error) = serve_connection(stream, tools, handler).await {
                        tracing::warn!(%error, "captain mcp connection exited with error");
                    }
                });
            }
            Err(error) => {
                tracing::warn!(%error, "captain mcp accept failed");
                break;
            }
        }
    }
}

pub async fn run_proxy(socket_path: PathBuf) -> io::Result<()> {
    let stream = UnixStream::connect(socket_path).await?;
    let (mut socket_reader, mut socket_writer) = stream.into_split();
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    let stdin_to_socket = tokio::spawn(async move {
        tokio::io::copy(&mut stdin, &mut socket_writer).await?;
        socket_writer.shutdown().await
    });
    let socket_to_stdout = tokio::spawn(async move {
        tokio::io::copy(&mut socket_reader, &mut stdout).await?;
        stdout.flush().await
    });

    let _ = stdin_to_socket.await;
    let _ = socket_to_stdout.await;
    Ok(())
}

async fn serve_connection(
    stream: UnixStream,
    tools: Arc<Vec<ToolDefinition>>,
    handler: ToolHandler,
) -> io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    while let Some(message) = read_message(&mut reader).await? {
        let request: JsonRpcRequest = match serde_json::from_value(message) {
            Ok(request) => request,
            Err(error) => {
                write_message(
                    &mut writer,
                    &json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32700,
                            "message": error.to_string(),
                        }
                    }),
                )
                .await?;
                continue;
            }
        };

        let Some(id) = request.id.clone() else {
            continue;
        };

        let response = match request.method.as_str() {
            "initialize" => {
                let params: InitializeParams =
                    serde_json::from_value(request.params).unwrap_or(InitializeParams {
                        protocol_version: None,
                    });
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": params.protocol_version.unwrap_or_else(|| "2025-03-26".to_owned()),
                        "capabilities": {
                            "tools": {
                                "listChanged": false
                            }
                        },
                        "serverInfo": {
                            "name": "ship",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                })
            }
            "tools/list" => {
                let tools = tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "name": tool.name,
                            "description": tool.description,
                            "inputSchema": tool.input_schema,
                        })
                    })
                    .collect::<Vec<_>>();
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tools }
                })
            }
            "tools/call" => {
                let params: ToolCallParams =
                    serde_json::from_value(request.params).unwrap_or(ToolCallParams {
                        name: String::new(),
                        arguments: Value::Null,
                    });
                let result = handler(params.name, params.arguments).await;
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.text }],
                        "isError": result.is_error,
                    }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("method not found: {}", request.method),
                }
            }),
        };

        write_message(&mut writer, &response).await?;
    }

    Ok(())
}

async fn read_message<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(content_length) = content_length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing Content-Length header",
        ));
    };

    let mut payload = vec![0; content_length];
    reader.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload).map(Some).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid json-rpc payload: {error}"),
        )
    })
}

async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, value: &Value) -> io::Result<()> {
    let payload = serde_json::to_vec(value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to encode json-rpc payload: {error}"),
        )
    })?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes())
        .await?;
    writer.write_all(&payload).await?;
    writer.flush().await
}
