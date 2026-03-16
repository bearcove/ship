/// Smoke test: launch an ACP adapter, initialize, create session, send one prompt.
/// Exits 0 on success, 1 on failure. Prints detailed diagnostics to stderr.
///
/// Usage:
///   cargo run --example smoke -p facet-acp -- claude
///   cargo run --example smoke -p facet-acp -- codex
///   cargo run --example smoke -p facet-acp -- opencode
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use facet_acp::*;

struct SmokeClient {
    got_response: Arc<AtomicBool>,
}

#[async_trait::async_trait(?Send)]
impl Client for SmokeClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> Result<RequestPermissionResponse> {
        eprintln!("  [smoke] permission requested, auto-approving");
        let option_id = args
            .options
            .first()
            .map(|o| o.option_id.clone())
            .unwrap_or_else(|| PermissionOptionId::new("allow"));
        Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id)),
        ))
    }

    async fn session_notification(&self, args: SessionNotification) -> Result<()> {
        match &args.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = &chunk.content {
                    eprint!("{}", text.text);
                    self.got_response.store(true, Ordering::Relaxed);
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let ContentBlock::Text(text) = &chunk.content {
                    eprint!("\x1b[2m{}\x1b[0m", text.text);
                }
            }
            SessionUpdate::ToolCall(tc) => {
                eprintln!("  [smoke] tool call: {}", tc.title);
            }
            SessionUpdate::ToolCallUpdate(update) => {
                if let Some(title) = &update.title {
                    eprintln!("  [smoke] tool update: {title}");
                }
            }
            SessionUpdate::UsageUpdate(usage) => {
                eprintln!(
                    "  [smoke] usage: {}/{}k tokens",
                    usage.used / 1000,
                    usage.size / 1000
                );
            }
            other => {
                eprintln!("  [smoke] session update: {other:?}");
            }
        }
        Ok(())
    }

    async fn write_text_file(&self, args: WriteTextFileRequest) -> Result<WriteTextFileResponse> {
        eprintln!("  [smoke] write_text_file: {}", args.path);
        Ok(WriteTextFileResponse { meta: None })
    }

    async fn read_text_file(&self, args: ReadTextFileRequest) -> Result<ReadTextFileResponse> {
        eprintln!("  [smoke] read_text_file: {}", args.path);
        Ok(ReadTextFileResponse {
            content: String::new(),
            meta: None,
        })
    }

    async fn create_terminal(
        &self,
        _args: CreateTerminalRequest,
    ) -> Result<CreateTerminalResponse> {
        Err(Error::method_not_found())
    }

    async fn terminal_output(
        &self,
        _args: TerminalOutputRequest,
    ) -> Result<TerminalOutputResponse> {
        Err(Error::method_not_found())
    }

    async fn release_terminal(
        &self,
        _args: ReleaseTerminalRequest,
    ) -> Result<ReleaseTerminalResponse> {
        Err(Error::method_not_found())
    }

    async fn wait_for_terminal_exit(
        &self,
        _args: WaitForTerminalExitRequest,
    ) -> Result<WaitForTerminalExitResponse> {
        Err(Error::method_not_found())
    }

    async fn kill_terminal_command(
        &self,
        _args: KillTerminalCommandRequest,
    ) -> Result<KillTerminalCommandResponse> {
        Err(Error::method_not_found())
    }

    async fn ext_method(&self, _args: ExtRequest) -> Result<ExtResponse> {
        Err(Error::method_not_found())
    }

    async fn ext_notification(&self, _args: ExtNotification) -> Result<()> {
        Ok(())
    }
}

struct AgentAdapter {
    binary: &'static str,
    pnpx_package: Option<&'static str>,
    args: &'static [&'static str],
}

const AGENTS: &[(&str, AgentAdapter)] = &[
    (
        "claude",
        AgentAdapter {
            binary: "claude-agent-acp",
            pnpx_package: Some("@zed-industries/claude-agent-acp"),
            args: &[],
        },
    ),
    (
        "codex",
        AgentAdapter {
            binary: "codex-acp",
            pnpx_package: Some("@zed-industries/codex-acp"),
            args: &[],
        },
    ),
    (
        "opencode",
        AgentAdapter {
            binary: "opencode",
            pnpx_package: None,
            args: &["acp"],
        },
    ),
];

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // RUST_LOG=trace to see wire-level JSON
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let local = tokio::task::LocalSet::new();
    let code = local
        .run_until(async {
            match run().await {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("FAIL: {e}");
                    1
                }
            }
        })
        .await;
    std::process::exit(code);
}

async fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let agent_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "claude".to_owned());

    let adapter = AGENTS
        .iter()
        .find(|(name, _)| *name == agent_name)
        .map(|(_, a)| a)
        .ok_or_else(|| {
            let names: Vec<_> = AGENTS.iter().map(|(n, _)| *n).collect();
            format!(
                "unknown agent '{}'. available: {}",
                agent_name,
                names.join(", ")
            )
        })?;

    let (program, mut extra_args) = if which_exists(adapter.binary) {
        (adapter.binary.to_owned(), vec![])
    } else if let Some(pkg) = adapter.pnpx_package {
        if which_exists("pnpx") {
            ("pnpx".to_owned(), vec![pkg.to_owned()])
        } else {
            return Err(format!(
                "neither '{}' nor 'pnpx' found in PATH",
                adapter.binary,
            )
            .into());
        }
    } else {
        return Err(format!("'{}' not found in PATH", adapter.binary).into());
    };

    for arg in adapter.args {
        extra_args.push(arg.to_string());
    }

    eprintln!("=== smoke test: {agent_name} ===");
    eprintln!("launching: {program} {}", extra_args.join(" "));

    let cwd = std::env::current_dir()?;

    let mut cmd = tokio::process::Command::new(&program);
    cmd.args(&extra_args)
        .current_dir(&cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    // The claude-agent-acp adapter needs CLAUDE_CODE_EXECUTABLE to find the
    // claude binary. If not already set, look it up in PATH.
    if std::env::var("CLAUDE_CODE_EXECUTABLE").is_err() {
        if let Some(path) = which_path("claude") {
            cmd.env("CLAUDE_CODE_EXECUTABLE", path);
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn '{program}': {e}"))?;

    let child_stdin = child.stdin.take().expect("stdin");
    let child_stdout = child.stdout.take().expect("stdout");
    let child_stderr = child.stderr.take().expect("stderr");

    // Drain adapter stderr in background, prefix each line
    tokio::task::spawn_local(async move {
        use tokio::io::AsyncBufReadExt;
        let mut reader = tokio::io::BufReader::new(child_stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => eprint!("  [adapter stderr] {}", line),
                Err(_) => break,
            }
        }
    });

    let got_response = Arc::new(AtomicBool::new(false));
    let client = SmokeClient {
        got_response: got_response.clone(),
    };

    let (connection, io_task) =
        ClientSideConnection::new(client, child_stdin, child_stdout, |future| {
            tokio::task::spawn_local(future);
        });

    tokio::task::spawn_local(async move {
        if let Err(e) = io_task.await {
            eprintln!("  [smoke] io task error: {e}");
        }
    });

    // Step 1: Initialize
    eprintln!("\n--- step 1: initialize ---");
    let init_req = InitializeRequest::new(ProtocolVersion::LATEST).client_capabilities(
        ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        },
    ).client_info(Implementation::new("facet-acp-smoke", env!("CARGO_PKG_VERSION")));
    let init_resp = connection.initialize(init_req).await?;
    eprintln!("  protocol_version: {}", init_resp.protocol_version);
    if init_resp.protocol_version.0 == 0 {
        return Err("protocol_version is 0 — likely failed to parse".into());
    }
    if let Some(info) = &init_resp.agent_info {
        eprintln!("  agent: {} v{}", info.name, info.version);
        if info.name.is_empty() {
            return Err("agent_info.name is empty".into());
        }
    } else {
        eprintln!("  WARN: no agent_info in response");
    }
    eprintln!("  PASS: initialize");

    // Step 2: New session
    eprintln!("\n--- step 2: new_session ---");
    let session_req = NewSessionRequest::new(cwd.to_string_lossy().to_string());
    let session_resp = connection.new_session(session_req).await?;
    let session_id = session_resp.session_id;
    eprintln!("  session_id: {session_id}");
    if session_id.0.is_empty() {
        return Err("session_id is empty".into());
    }
    if let Some(opts) = &session_resp.config_options {
        eprintln!("  config_options: {} entries", opts.len());
        for opt in opts {
            eprintln!("    - {} (id: {})", opt.name, opt.id.0);
        }
    }
    if let Some(models) = &session_resp.models {
        eprintln!("  current_model: {}", models.current_model_id.0);
        eprintln!("  available_models: {}", models.available_models.len());
    }
    if let Some(modes) = &session_resp.modes {
        eprintln!("  current_mode: {}", modes.current_mode_id.0);
        eprintln!("  available_modes: {}", modes.available_modes.len());
    }
    eprintln!("  PASS: new_session");

    // Step 3: Send a prompt
    eprintln!("\n--- step 3: prompt ---");
    let prompt_req = PromptRequest::new(
        session_id.clone(),
        vec![ContentBlock::from(
            "Reply with exactly the word 'hello' and nothing else.",
        )],
    );

    // Race the prompt against a timeout
    let prompt_result = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        connection.prompt(prompt_req),
    )
    .await;

    match prompt_result {
        Ok(Ok(resp)) => {
            eprintln!("\n  stop_reason: {:?}", resp.stop_reason);
            eprintln!("  PASS: prompt");
        }
        Ok(Err(e)) => {
            eprintln!("\n  prompt error: {e}");
            return Err(format!("prompt failed: {e}").into());
        }
        Err(_) => {
            eprintln!("\n  prompt timed out after 60s");
            return Err("prompt timed out".into());
        }
    }

    // Verify we got some response text
    if got_response.load(Ordering::Relaxed) {
        eprintln!("  PASS: got agent response text");
    } else {
        eprintln!("  WARN: no agent message chunks received");
    }

    // Cleanup
    let _ = connection
        .cancel(CancelNotification::new(session_id))
        .await;
    let _ = child.kill().await;

    eprintln!("\n=== smoke test PASSED ===");
    Ok(())
}

fn which_exists(name: &str) -> bool {
    which_path(name).is_some()
}

fn which_path(name: &str) -> Option<String> {
    let output = std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
