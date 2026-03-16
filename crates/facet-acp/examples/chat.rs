use std::io::{self, BufRead, Write};

use facet_acp::*;

/// Minimal Client implementation that prints agent output to stderr.
struct PrintingClient;

#[async_trait::async_trait(?Send)]
impl Client for PrintingClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> Result<RequestPermissionResponse> {
        // Auto-approve everything — this is a demo
        let option_id = args
            .options
            .iter()
            .find(|o| matches!(o.kind, PermissionOptionKind::AllowOnce))
            .or_else(|| args.options.first())
            .map(|o| o.option_id.clone())
            .unwrap_or_else(|| PermissionOptionId::new("allow"));

        Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id)),
        ))
    }

    async fn session_notification(&self, args: SessionNotification) -> Result<()> {
        match args.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    eprint!("{}", text.text);
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    eprint!("\x1b[2m{}\x1b[0m", text.text);
                }
            }
            SessionUpdate::ToolCall(tc) => {
                eprintln!("\n\x1b[33m⚡ {}\x1b[0m", tc.title);
            }
            SessionUpdate::ToolCallUpdate(update) => {
                if let Some(ToolCallStatus::Completed) = update.status {
                    if let Some(title) = &update.title {
                        eprintln!("\x1b[32m✓ {title}\x1b[0m");
                    }
                }
            }
            SessionUpdate::UsageUpdate(usage) => {
                eprintln!(
                    "\n\x1b[2m[{}/{}k tokens]\x1b[0m",
                    usage.used / 1000,
                    usage.size / 1000
                );
            }
            _ => {}
        }
        Ok(())
    }

    async fn write_text_file(&self, args: WriteTextFileRequest) -> Result<WriteTextFileResponse> {
        eprintln!("\x1b[33m📝 write {}\x1b[0m", args.path);
        std::fs::write(&args.path, &args.content)
            .map_err(|e| Error::internal_error().data(e.to_string()))?;
        Ok(WriteTextFileResponse { meta: None })
    }

    async fn read_text_file(&self, args: ReadTextFileRequest) -> Result<ReadTextFileResponse> {
        let content = std::fs::read_to_string(&args.path)
            .map_err(|e| Error::resource_not_found(Some(args.path.clone())).data(e.to_string()))?;
        Ok(ReadTextFileResponse { content, meta: None })
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

/// ACP agent adapter binaries and their fallbacks.
struct AgentAdapter {
    /// Primary binary name (e.g. "claude-agent-acp")
    binary: &'static str,
    /// pnpx package fallback (e.g. "@zed-industries/claude-agent-acp")
    pnpx_package: Option<&'static str>,
    /// Extra args to pass after the binary/package
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
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            if let Err(e) = run().await {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        })
        .await;
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

    // Resolve the actual command: try direct binary first, then pnpx fallback
    let (program, mut extra_args) = if which_exists(adapter.binary) {
        (adapter.binary.to_owned(), vec![])
    } else if let Some(pkg) = adapter.pnpx_package {
        if which_exists("pnpx") {
            ("pnpx".to_owned(), vec![pkg.to_owned()])
        } else {
            return Err(format!(
                "neither '{}' nor 'pnpx' found in PATH. install the ACP adapter:\n  npm install -g {}",
                adapter.binary,
                pkg,
            )
            .into());
        }
    } else {
        return Err(format!("'{}' not found in PATH", adapter.binary).into());
    };

    for arg in adapter.args {
        extra_args.push(arg.to_string());
    }

    eprintln!("launching: {} {}", program, extra_args.join(" "));

    let cwd = std::env::current_dir()?;

    // Spawn the ACP adapter process
    let mut cmd = tokio::process::Command::new(&program);
    cmd.args(&extra_args)
        .current_dir(&cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true);

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

    let client = PrintingClient;

    let (connection, io_task) = ClientSideConnection::new(client, child_stdin, child_stdout, |future| {
        tokio::task::spawn_local(future);
    });

    // Spawn the I/O read loop
    tokio::task::spawn_local(async move {
        if let Err(e) = io_task.await {
            eprintln!("io task error: {e}");
        }
    });

    // Initialize
    let init_req = InitializeRequest::new(ProtocolVersion::LATEST)
        .client_capabilities(ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: false,
            meta: None,
        })
        .client_info(Implementation::new(
            "facet-acp-chat",
            env!("CARGO_PKG_VERSION"),
        ));
    let init_resp = connection.initialize(init_req).await?;
    if let Some(info) = &init_resp.agent_info {
        eprintln!("connected to {} v{}", info.name, info.version);
    }
    eprintln!("protocol version: {}", init_resp.protocol_version);

    // Create a new session
    let session_req = NewSessionRequest::new(cwd.to_string_lossy().to_string());
    let session_resp = connection.new_session(session_req).await?;
    let session_id = session_resp.session_id;
    eprintln!("session: {session_id}");
    eprintln!("---");
    eprintln!("type your prompt, /quit to exit\n");

    // REPL
    let stdin = io::stdin();
    loop {
        eprint!("\x1b[1myou>\x1b[0m ");
        io::stderr().flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break; // EOF
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "/quit" || line == "/exit" {
            break;
        }

        let prompt_req = PromptRequest::new(
            session_id.clone(),
            vec![ContentBlock::from(line.to_owned())],
        );
        match connection.prompt(prompt_req).await {
            Ok(resp) => {
                eprintln!("\n\x1b[2m[stop: {:?}]\x1b[0m\n", resp.stop_reason);
            }
            Err(e) => {
                eprintln!("\nerror: {e}\n");
            }
        }
    }

    // Clean up
    let _ = connection
        .cancel(CancelNotification::new(session_id))
        .await;
    let _ = child.kill().await;

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
