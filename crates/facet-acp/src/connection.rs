use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, oneshot};

use crate::rpc::*;
use crate::schema::*;

/// A client-side connection to an agent, communicating over JSON-RPC on stdio.
///
/// Implements the `Agent` trait — call methods on this to talk to the agent process.
#[derive(Debug)]
pub struct ClientSideConnection {
    inner: Arc<ConnectionInner>,
}

struct ConnectionInner {
    next_id: AtomicI64,
    writer: Mutex<Box<dyn AsyncWriteSend>>,
    pending: Mutex<HashMap<i64, oneshot::Sender<RpcResult>>>,
}

impl std::fmt::Debug for ConnectionInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionInner")
            .field("next_id", &self.next_id)
            .finish_non_exhaustive()
    }
}

/// Trait alias so we can store the writer in a mutex.
trait AsyncWriteSend: tokio::io::AsyncWrite + Unpin + Send {}
impl<T: tokio::io::AsyncWrite + Unpin + Send> AsyncWriteSend for T {}

type RpcResult = std::result::Result<facet_json::RawJson<'static>, crate::Error>;

impl ClientSideConnection {
    /// Create a new client-side connection.
    ///
    /// Returns the connection (for making requests) and an I/O future that
    /// must be spawned to drive the read loop.
    ///
    /// `client` handles incoming requests from the agent (permission, file ops, etc).
    /// `outgoing_bytes` is the agent's stdin.
    /// `incoming_bytes` is the agent's stdout.
    /// `spawn` is called to spawn async tasks for handling incoming requests.
    pub fn new(
        client: impl Client + 'static,
        outgoing_bytes: impl tokio::io::AsyncWrite + Unpin + Send + 'static,
        incoming_bytes: impl tokio::io::AsyncRead + Unpin + Send + 'static,
        _spawn: impl Fn(LocalBoxFuture<'static, ()>) + 'static,
    ) -> (Self, impl std::future::Future<Output = crate::Result<()>>) {
        let inner = Arc::new(ConnectionInner {
            next_id: AtomicI64::new(1),
            writer: Mutex::new(Box::new(outgoing_bytes)),
            pending: Mutex::new(HashMap::new()),
        });

        let conn = Self {
            inner: inner.clone(),
        };

        let io_task = run_read_loop(inner, incoming_bytes, client, _spawn);

        (conn, io_task)
    }

    async fn request<R: for<'a> facet::Facet<'a>>(
        &self,
        method: &str,
        params: &(impl for<'a> facet::Facet<'a> + ?Sized),
    ) -> crate::Result<R> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);

        let params_json = facet_json::to_string(params)
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: Some(JsonRpcId::Number(id)),
            method: method.to_owned(),
            params: Some(facet_json::RawJson::from_owned(params_json)),
        };

        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id, tx);

        // Send the request
        let msg = facet_json::to_string(&request)
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;

        tracing::trace!(method, id, "→ {msg}");

        {
            let mut writer = self.inner.writer.lock().await;
            writer
                .write_all(msg.as_bytes())
                .await
                .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
            writer
                .write_all(b"\n")
                .await
                .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
            writer
                .flush()
                .await
                .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        }

        // Wait for the response
        let raw = rx
            .await
            .map_err(|_| crate::Error::internal_error().data("connection closed"))??;

        tracing::trace!(method, id, "← {}", raw.as_ref());

        facet_json::from_str(raw.as_ref())
            .map_err(|e| crate::Error::invalid_params().data(e.to_string()))
    }

    async fn notify(
        &self,
        method: &str,
        params: &(impl for<'a> facet::Facet<'a> + ?Sized),
    ) -> crate::Result<()> {
        let params_json = facet_json::to_string(params)
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: None,
            method: method.to_owned(),
            params: Some(facet_json::RawJson::from_owned(params_json)),
        };

        let msg = facet_json::to_string(&request)
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;

        tracing::trace!(method, "→ {msg}");

        let mut writer = self.inner.writer.lock().await;
        writer
            .write_all(msg.as_bytes())
            .await
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for ClientSideConnection {
    async fn initialize(&self, args: InitializeRequest) -> crate::Result<InitializeResponse> {
        self.request(AGENT_METHOD_NAMES.initialize, &args).await
    }

    async fn authenticate(
        &self,
        args: AuthenticateRequest,
    ) -> crate::Result<AuthenticateResponse> {
        self.request(AGENT_METHOD_NAMES.authenticate, &args).await
    }

    async fn new_session(&self, args: NewSessionRequest) -> crate::Result<NewSessionResponse> {
        self.request(AGENT_METHOD_NAMES.session_new, &args).await
    }

    async fn load_session(&self, args: LoadSessionRequest) -> crate::Result<LoadSessionResponse> {
        self.request(AGENT_METHOD_NAMES.session_load, &args).await
    }

    async fn set_session_mode(
        &self,
        args: SetSessionModeRequest,
    ) -> crate::Result<SetSessionModeResponse> {
        self.request(AGENT_METHOD_NAMES.session_set_mode, &args)
            .await
    }

    async fn prompt(&self, args: PromptRequest) -> crate::Result<crate::schema::PromptResponse> {
        self.request(AGENT_METHOD_NAMES.session_prompt, &args).await
    }

    async fn cancel(&self, args: CancelNotification) -> crate::Result<()> {
        self.notify(AGENT_METHOD_NAMES.session_cancel, &args).await
    }

    async fn set_session_model(
        &self,
        args: SetSessionModelRequest,
    ) -> crate::Result<SetSessionModelResponse> {
        self.request(AGENT_METHOD_NAMES.session_set_model, &args)
            .await
    }

    async fn resume_session(
        &self,
        args: ResumeSessionRequest,
    ) -> crate::Result<ResumeSessionResponse> {
        self.request(AGENT_METHOD_NAMES.session_resume, &args).await
    }

    async fn set_session_config_option(
        &self,
        args: SetSessionConfigOptionRequest,
    ) -> crate::Result<SetSessionConfigOptionResponse> {
        self.request(AGENT_METHOD_NAMES.session_set_config_option, &args)
            .await
    }

    async fn ext_method(&self, args: ExtRequest) -> crate::Result<ExtResponse> {
        let method = format!("_{}", args.method);
        // For ext methods, we send the raw params and get raw back
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: Some(JsonRpcId::Number(id)),
            method,
            params: Some(args.params),
        };

        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id, tx);

        let msg = facet_json::to_string(&request)
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;

        {
            let mut writer = self.inner.writer.lock().await;
            writer.write_all(msg.as_bytes()).await
                .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
            writer.write_all(b"\n").await
                .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
            writer.flush().await
                .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        }

        let raw = rx.await
            .map_err(|_| crate::Error::internal_error().data("connection closed"))??;
        Ok(ExtResponse::new(raw))
    }

    async fn ext_notification(&self, args: ExtNotification) -> crate::Result<()> {
        let method = format!("_{}", args.method);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: None,
            method,
            params: Some(args.params),
        };

        let msg = facet_json::to_string(&request)
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;

        let mut writer = self.inner.writer.lock().await;
        writer.write_all(msg.as_bytes()).await
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        writer.write_all(b"\n").await
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        writer.flush().await
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        Ok(())
    }
}

/// The read loop: reads JSON-RPC messages from the agent and dispatches them.
async fn run_read_loop(
    inner: Arc<ConnectionInner>,
    incoming: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    client: impl Client + 'static,
    _spawn: impl Fn(LocalBoxFuture<'static, ()>) + 'static,
) -> crate::Result<()> {
    let mut reader = BufReader::new(incoming);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| crate::Error::internal_error().data(e.to_string()))?;
        if n == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        tracing::trace!("← {trimmed}");

        // Try to figure out if this is a response (has result/error) or a request/notification
        // We parse as a generic JSON-RPC message first
        let msg: JsonRpcResponse = match facet_json::from_str(trimmed) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("failed to parse incoming message: {e}");
                continue;
            }
        };

        // If it has an id and (result or error), it's a response to one of our requests
        if let Some(JsonRpcId::Number(id)) = &msg.id {
            if msg.result.is_some() || msg.error.is_some() {
                let mut pending = inner.pending.lock().await;
                if let Some(tx) = pending.remove(id) {
                    let result = if let Some(raw) = msg.result {
                        Ok(raw)
                    } else if let Some(err) = msg.error {
                        Err(crate::Error::new(err.code, err.message))
                    } else {
                        Err(crate::Error::internal_error())
                    };
                    let _ = tx.send(result);
                }
                continue;
            }
        }

        // Otherwise it's an incoming request or notification from the agent.
        // Re-parse as a request to get the method.
        let req: JsonRpcRequest = match facet_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("failed to parse incoming request: {e}");
                continue;
            }
        };

        let method = req.method.clone();
        let params_raw = req.params;
        let request_id = req.id;
        let inner2 = inner.clone();

        // For notifications (no id), dispatch inline.
        // For requests (has id), spawn a task so we can send the response.
        match method.as_str() {
            m if m == CLIENT_METHOD_NAMES.session_update => {
                // Notification — no response needed
                if let Some(raw) = params_raw {
                    match facet_json::from_str::<SessionNotification>(raw.as_ref()) {
                        Ok(notification) => {
                            let _ = client.session_notification(notification).await;
                        }
                        Err(e) => {
                            tracing::warn!("failed to parse session notification: {e}");
                        }
                    }
                }
            }
            m if m == CLIENT_METHOD_NAMES.session_request_permission => {
                if let (Some(raw), Some(id)) = (params_raw, request_id) {
                    // Need to spawn because this blocks waiting for user input
                    // We can't move `client` into the spawned future since it's borrowed,
                    // so we handle it inline for now.
                    match facet_json::from_str::<RequestPermissionRequest>(raw.as_ref()) {
                        Ok(req) => {
                            let resp = client.request_permission(req).await;
                            send_response(&inner2, id, resp).await;
                        }
                        Err(e) => {
                            let err = crate::Error::invalid_params().data(e.to_string());
                            send_response::<RequestPermissionResponse>(&inner2, id, Err(err))
                                .await;
                        }
                    }
                }
            }
            m if m == CLIENT_METHOD_NAMES.fs_read_text_file => {
                if let (Some(raw), Some(id)) = (params_raw, request_id) {
                    match facet_json::from_str::<ReadTextFileRequest>(raw.as_ref()) {
                        Ok(req) => {
                            let resp = client.read_text_file(req).await;
                            send_response(&inner2, id, resp).await;
                        }
                        Err(e) => {
                            let err = crate::Error::invalid_params().data(e.to_string());
                            send_response::<ReadTextFileResponse>(&inner2, id, Err(err)).await;
                        }
                    }
                }
            }
            m if m == CLIENT_METHOD_NAMES.fs_write_text_file => {
                if let (Some(raw), Some(id)) = (params_raw, request_id) {
                    match facet_json::from_str::<WriteTextFileRequest>(raw.as_ref()) {
                        Ok(req) => {
                            let resp = client.write_text_file(req).await;
                            send_response(&inner2, id, resp).await;
                        }
                        Err(e) => {
                            let err = crate::Error::invalid_params().data(e.to_string());
                            send_response::<WriteTextFileResponse>(&inner2, id, Err(err)).await;
                        }
                    }
                }
            }
            m if m == CLIENT_METHOD_NAMES.terminal_create => {
                if let (Some(raw), Some(id)) = (params_raw, request_id) {
                    match facet_json::from_str::<CreateTerminalRequest>(raw.as_ref()) {
                        Ok(req) => {
                            let resp = client.create_terminal(req).await;
                            send_response(&inner2, id, resp).await;
                        }
                        Err(e) => {
                            let err = crate::Error::invalid_params().data(e.to_string());
                            send_response::<CreateTerminalResponse>(&inner2, id, Err(err)).await;
                        }
                    }
                }
            }
            m if m == CLIENT_METHOD_NAMES.terminal_output => {
                if let (Some(raw), Some(id)) = (params_raw, request_id) {
                    match facet_json::from_str::<TerminalOutputRequest>(raw.as_ref()) {
                        Ok(req) => {
                            let resp = client.terminal_output(req).await;
                            send_response(&inner2, id, resp).await;
                        }
                        Err(e) => {
                            let err = crate::Error::invalid_params().data(e.to_string());
                            send_response::<TerminalOutputResponse>(&inner2, id, Err(err)).await;
                        }
                    }
                }
            }
            m if m == CLIENT_METHOD_NAMES.terminal_kill => {
                if let (Some(raw), Some(id)) = (params_raw, request_id) {
                    match facet_json::from_str::<KillTerminalCommandRequest>(raw.as_ref()) {
                        Ok(req) => {
                            let resp = client.kill_terminal_command(req).await;
                            send_response(&inner2, id, resp).await;
                        }
                        Err(e) => {
                            let err = crate::Error::invalid_params().data(e.to_string());
                            send_response::<KillTerminalCommandResponse>(&inner2, id, Err(err))
                                .await;
                        }
                    }
                }
            }
            m if m == CLIENT_METHOD_NAMES.terminal_release => {
                if let (Some(raw), Some(id)) = (params_raw, request_id) {
                    match facet_json::from_str::<ReleaseTerminalRequest>(raw.as_ref()) {
                        Ok(req) => {
                            let resp = client.release_terminal(req).await;
                            send_response(&inner2, id, resp).await;
                        }
                        Err(e) => {
                            let err = crate::Error::invalid_params().data(e.to_string());
                            send_response::<ReleaseTerminalResponse>(&inner2, id, Err(err)).await;
                        }
                    }
                }
            }
            m if m == CLIENT_METHOD_NAMES.terminal_wait_for_exit => {
                if let (Some(raw), Some(id)) = (params_raw, request_id) {
                    match facet_json::from_str::<WaitForTerminalExitRequest>(raw.as_ref()) {
                        Ok(req) => {
                            let resp = client.wait_for_terminal_exit(req).await;
                            send_response(&inner2, id, resp).await;
                        }
                        Err(e) => {
                            let err = crate::Error::invalid_params().data(e.to_string());
                            send_response::<WaitForTerminalExitResponse>(&inner2, id, Err(err))
                                .await;
                        }
                    }
                }
            }
            other => {
                // Extension methods start with _
                if let Some(custom_method) = other.strip_prefix('_') {
                    if let Some(raw) = params_raw {
                        if let Some(id) = request_id {
                            // Extension request
                            let ext_req = ExtRequest::new(custom_method, raw);
                            let resp = client.ext_method(ext_req).await;
                            match resp {
                                Ok(ext_resp) => {
                                    send_raw_response(&inner2, id, Ok(ext_resp.0)).await;
                                }
                                Err(e) => {
                                    send_raw_response(&inner2, id, Err(e)).await;
                                }
                            }
                        } else {
                            // Extension notification
                            let ext_notif = ExtNotification::new(custom_method, raw);
                            let _ = client.ext_notification(ext_notif).await;
                        }
                    }
                } else {
                    tracing::warn!("unknown method: {other}");
                }
            }
        }
    }

    Ok(())
}

async fn send_response<R: for<'a> facet::Facet<'a>>(
    inner: &ConnectionInner,
    id: JsonRpcId,
    result: crate::Result<R>,
) {
    match result {
        Ok(value) => {
            let json = match facet_json::to_string(&value) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!("failed to serialize response: {e}");
                    return;
                }
            };
            send_raw_response(inner, id, Ok(facet_json::RawJson::from_owned(json))).await;
        }
        Err(e) => {
            send_raw_response(inner, id, Err(e)).await;
        }
    }
}

async fn send_raw_response(
    inner: &ConnectionInner,
    id: JsonRpcId,
    result: std::result::Result<facet_json::RawJson<'static>, crate::Error>,
) {
    let response = match result {
        Ok(raw) => JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id: Some(id),
            result: Some(raw),
            error: None,
        },
        Err(e) => JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id: Some(id),
            result: None,
            error: Some(e.into()),
        },
    };

    let msg = match facet_json::to_string(&response) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("failed to serialize response: {e}");
            return;
        }
    };

    let mut writer = inner.writer.lock().await;
    if let Err(e) = writer.write_all(msg.as_bytes()).await {
        tracing::error!("failed to write response: {e}");
        return;
    }
    if let Err(e) = writer.write_all(b"\n").await {
        tracing::error!("failed to write newline: {e}");
        return;
    }
    let _ = writer.flush().await;
}
