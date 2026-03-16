use facet::Facet;
use facet_json::RawJson;

// ── JSON-RPC 2.0 framing ────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(untagged)]
#[repr(u8)]
pub enum JsonRpcId {
    Number(i64),
    Str(String),
}

/// An incoming JSON-RPC message (request or notification).
#[derive(Debug, Facet)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[facet(default)]
    pub id: Option<JsonRpcId>,
    pub method: String,
    #[facet(default)]
    pub params: Option<RawJson<'static>>,
}

/// An outgoing JSON-RPC response.
#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[facet(default)]
    pub id: Option<JsonRpcId>,
    #[facet(default)]
    pub result: Option<RawJson<'static>>,
    #[facet(default)]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Facet)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[facet(default)]
    pub data: Option<String>,
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
