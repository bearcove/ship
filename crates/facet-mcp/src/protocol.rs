use facet::Facet;
use facet_json_schema::JsonSchema;

// ── JSON-RPC 2.0 framing ────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    pub method: String,
    #[facet(default)]
    pub params: Option<String>,
}

#[derive(Debug, Clone, Facet)]
#[facet(untagged)]
#[repr(u8)]
pub enum JsonRpcId {
    Number(i64),
    Str(String),
}

#[derive(Debug, Facet)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[facet(default)]
    pub id: Option<JsonRpcId>,
    #[facet(default)]
    pub result: Option<String>,
    #[facet(default)]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Facet)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

// ── MCP protocol types ──────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: Implementation,
}

#[derive(Debug, Facet)]
pub struct ClientCapabilities {}

#[derive(Debug, Facet)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Facet)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Facet)]
pub struct ServerCapabilities {
    #[facet(default)]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Facet)]
pub struct ToolsCapability {
    #[facet(default, rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Facet)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Facet)]
pub struct ListToolsResult {
    pub tools: Vec<ToolInfo>,
}

#[derive(Debug, Facet)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    #[facet(rename = "inputSchema")]
    pub input_schema: JsonSchema,
}

#[derive(Debug, Facet)]
pub struct CallToolParams {
    pub name: String,
    #[facet(default)]
    pub arguments: Option<String>,
}

#[derive(Debug, Facet)]
pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
    #[facet(default, rename = "isError")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Facet)]
#[repr(u8)]
pub enum ContentBlock {
    Text { text: String },
}

impl CallToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text { text: text.into() }],
            is_error: None,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text { text: text.into() }],
            is_error: Some(true),
        }
    }
}
