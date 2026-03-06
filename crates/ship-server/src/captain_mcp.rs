use std::fs::OpenOptions;
use std::future::Future;
use std::io::Write as _;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

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
                tracing::info!("captain mcp accepted connection");
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
    let proxy_started_at = Instant::now();
    let log_path = socket_path.with_extension("proxy.log");
    append_proxy_log(
        &log_path,
        &format!(
            "proxy start socket={} elapsed_ms={}",
            socket_path.display(),
            proxy_started_at.elapsed().as_millis()
        ),
    );
    tracing::info!(socket = %socket_path.display(), "captain mcp proxy connecting to unix socket");
    let stream = UnixStream::connect(socket_path).await?;
    append_proxy_log(
        &log_path,
        &format!(
            "proxy connected elapsed_ms={}",
            proxy_started_at.elapsed().as_millis()
        ),
    );
    tracing::info!("captain mcp proxy connected to unix socket");
    let (mut socket_reader, mut socket_writer) = stream.into_split();
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    let stdin_to_socket_log_path = log_path.clone();
    let stdin_to_socket = tokio::spawn(async move {
        copy_with_first_chunk_log(
            &mut stdin,
            &mut socket_writer,
            "captain mcp proxy stdin->socket",
            proxy_started_at,
            &stdin_to_socket_log_path,
        )
        .await?;
        socket_writer.shutdown().await
    });
    let socket_to_stdout_log_path = log_path.clone();
    let socket_to_stdout = tokio::spawn(async move {
        copy_with_first_chunk_log(
            &mut socket_reader,
            &mut stdout,
            "captain mcp proxy socket->stdout",
            proxy_started_at,
            &socket_to_stdout_log_path,
        )
        .await?;
        stdout.flush().await
    });

    let _ = stdin_to_socket.await;
    let _ = socket_to_stdout.await;
    Ok(())
}

async fn copy_with_first_chunk_log<R, W>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    started_at: Instant,
    log_path: &std::path::Path,
) -> io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut total = 0u64;
    let mut first_chunk_logged = false;
    let mut buf = vec![0u8; 8 * 1024];

    loop {
        let read = reader.read(&mut buf).await?;
        if read == 0 {
            append_proxy_log(
                log_path,
                &format!(
                    "{label} eof total_bytes={} elapsed_ms={}",
                    total,
                    started_at.elapsed().as_millis()
                ),
            );
            eprintln!(
                "[ship debug] {label} eof total_bytes={} elapsed_ms={}",
                total,
                started_at.elapsed().as_millis()
            );
            return Ok(total);
        }

        if !first_chunk_logged {
            first_chunk_logged = true;
            let preview = first_chunk_preview(&buf[..read]);
            append_proxy_log(
                log_path,
                &format!(
                    "{label} first_chunk_bytes={} elapsed_ms={} preview={preview}",
                    read,
                    started_at.elapsed().as_millis()
                ),
            );
            eprintln!(
                "[ship debug] {label} first_chunk_bytes={} elapsed_ms={} preview={preview}",
                read,
                started_at.elapsed().as_millis()
            );
        }

        writer.write_all(&buf[..read]).await?;
        total += read as u64;
    }
}

fn append_proxy_log(path: &std::path::Path, message: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

fn first_chunk_preview(bytes: &[u8]) -> String {
    let hex = bytes
        .iter()
        .take(64)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("");
    let ascii = bytes
        .iter()
        .take(64)
        .map(|byte| match byte {
            b'\r' => "\\r".to_owned(),
            b'\n' => "\\n".to_owned(),
            b'\t' => "\\t".to_owned(),
            0x20..=0x7e => (*byte as char).to_string(),
            _ => ".".to_owned(),
        })
        .collect::<String>();
    format!("hex={hex} ascii={ascii:?}")
}

async fn serve_connection(
    stream: UnixStream,
    tools: Arc<Vec<ToolDefinition>>,
    handler: ToolHandler,
) -> io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    while let Some(message) = read_message(&mut reader).await? {
        let request_started_at = Instant::now();
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
        tracing::info!(method = request.method, "captain mcp received request");

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
        tracing::info!(
            method = request.method,
            elapsed_ms = request_started_at.elapsed().as_millis(),
            "captain mcp sent response"
        );
    }

    Ok(())
}

async fn read_message<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let read_started_at = Instant::now();
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
    tracing::info!(
        content_length,
        elapsed_ms = read_started_at.elapsed().as_millis(),
        "captain mcp received message headers"
    );

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
