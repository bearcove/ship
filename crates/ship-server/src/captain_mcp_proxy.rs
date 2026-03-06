use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::UnixStream;

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
