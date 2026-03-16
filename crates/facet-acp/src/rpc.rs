use facet::Facet;
use facet_json::RawJson;

// ── JSON-RPC 2.0 framing ────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(untagged)]
#[repr(u8)]
pub enum JsonRpcId {
    Null {},
    Number(i64),
    Str(String),
}

/// A JSON-RPC 2.0 message — can be a request, notification, or response.
///
/// Routing rules:
/// - Has `method` + `id` → request (expects a response)
/// - Has `method`, no `id` → notification (fire-and-forget)
/// - Has `id` + (`result` or `error`), no `method` → response
#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    #[facet(default, skip_unless_truthy)]
    pub id: Option<JsonRpcId>,
    #[facet(default, skip_unless_truthy)]
    pub method: Option<String>,
    #[facet(default, skip_unless_truthy)]
    pub params: Option<RawJson<'static>>,
    #[facet(default, skip_unless_truthy)]
    pub result: Option<RawJson<'static>>,
    #[facet(default, skip_unless_truthy)]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcMessage {
    /// Build an outgoing request (has method + id).
    pub fn request(id: i64, method: impl Into<String>, params: RawJson<'static>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: Some(JsonRpcId::Number(id)),
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Build an outgoing notification (has method, no id).
    pub fn notification(method: impl Into<String>, params: RawJson<'static>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: None,
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Build an outgoing success response.
    pub fn response_ok(id: JsonRpcId, result: RawJson<'static>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: Some(id),
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    /// Build an outgoing error response.
    pub fn response_err(id: JsonRpcId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: Some(id),
            method: None,
            params: None,
            result: None,
            error: Some(error),
        }
    }

    /// Is this a response? (has result or error, no method)
    pub fn is_response(&self) -> bool {
        self.method.is_none() && (self.result.is_some() || self.error.is_some())
    }

    /// Is this a request? (has method and id)
    pub fn is_request(&self) -> bool {
        self.method.is_some() && self.id.is_some()
    }

    /// Is this a notification? (has method, no id)
    pub fn is_notification(&self) -> bool {
        self.method.is_some() && self.id.is_none()
    }
}

#[derive(Debug, Facet)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[facet(default, skip_unless_truthy)]
    pub data: Option<RawJson<'static>>,
}

impl From<crate::Error> for JsonRpcError {
    fn from(e: crate::Error) -> Self {
        Self {
            code: e.code,
            message: e.message,
            data: e.data,
        }
    }
}

/// ACP method names for client→agent requests.
pub struct AgentMethodNames {
    pub initialize: &'static str,
    pub authenticate: &'static str,
    pub session_new: &'static str,
    pub session_load: &'static str,
    pub session_set_mode: &'static str,
    pub session_prompt: &'static str,
    pub session_cancel: &'static str,
    pub session_set_model: &'static str,
    pub session_resume: &'static str,
    pub session_set_config_option: &'static str,
}

pub const AGENT_METHOD_NAMES: AgentMethodNames = AgentMethodNames {
    initialize: "initialize",
    authenticate: "authenticate",
    session_new: "session/new",
    session_load: "session/load",
    session_set_mode: "session/setMode",
    session_prompt: "session/prompt",
    session_cancel: "session/cancel",
    session_set_model: "session/setModel",
    session_resume: "session/resume",
    session_set_config_option: "session/setConfigOption",
};

/// ACP method names for agent→client requests/notifications.
pub struct ClientMethodNames {
    pub session_update: &'static str,
    pub session_request_permission: &'static str,
    pub fs_write_text_file: &'static str,
    pub fs_read_text_file: &'static str,
    pub terminal_create: &'static str,
    pub terminal_output: &'static str,
    pub terminal_kill: &'static str,
    pub terminal_release: &'static str,
    pub terminal_wait_for_exit: &'static str,
}

pub const CLIENT_METHOD_NAMES: ClientMethodNames = ClientMethodNames {
    session_update: "session/update",
    session_request_permission: "session/requestPermission",
    fs_write_text_file: "fs/writeTextFile",
    fs_read_text_file: "fs/readTextFile",
    terminal_create: "terminal/create",
    terminal_output: "terminal/output",
    terminal_kill: "terminal/kill",
    terminal_release: "terminal/release",
    terminal_wait_for_exit: "terminal/waitForExit",
};
