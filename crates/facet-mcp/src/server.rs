use std::fmt;

use facet_json_schema::JsonSchema;

use crate::protocol::{
    CallToolResult, InitializeResult, ListToolsResult, ServerCapabilities, ServerInfo,
    ToolInfo, ToolsCapability,
};
use crate::transport::{StdioTransport, TransportError};

/// Static definition of a tool: name, description, and JSON Schema for its input.
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: JsonSchema,
}

/// A tool's result: either success text or error text.
pub type ToolResult = CallToolResult;

/// Implement this to handle tool calls.
pub trait ToolHandler: Send + Sync + 'static {
    /// Return the list of tools this handler provides.
    fn tool_defs(&self) -> &[ToolDef];

    /// Dispatch a tool call. `arguments` is the raw JSON of the arguments object.
    fn call_tool(
        &self,
        name: &str,
        arguments: Option<&facet_json::RawJson<'static>>,
    ) -> impl std::future::Future<Output = CallToolResult> + Send;
}

#[derive(Debug)]
pub enum ServerError {
    Transport(TransportError),
    SerializeFailed(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "{e}"),
            Self::SerializeFailed(msg) => write!(f, "failed to serialize MCP result: {msg}"),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<TransportError> for ServerError {
    fn from(e: TransportError) -> Self {
        Self::Transport(e)
    }
}

pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

/// An MCP server that reads JSON-RPC from stdin and writes to stdout.
pub struct McpServer<H> {
    handler: H,
    info: McpServerInfo,
}

impl<H: ToolHandler> McpServer<H> {
    pub fn new(handler: H, info: McpServerInfo) -> Self {
        Self { handler, info }
    }

    fn serialize<T: for<'a> facet::Facet<'a>>(&self, value: &T) -> Result<String, ServerError> {
        facet_json::to_string(value)
            .map_err(|e| ServerError::SerializeFailed(e.to_string()))
    }

    pub async fn run(self) -> Result<(), ServerError> {
        let mut transport = StdioTransport::new();

        loop {
            let Some(request) = transport.read_request().await? else {
                break;
            };

            let response_result = match request.method.as_str() {
                "initialize" => {
                    let result = InitializeResult {
                        protocol_version: "2025-03-26".to_owned(),
                        capabilities: ServerCapabilities {
                            tools: Some(ToolsCapability {
                                list_changed: Some(false),
                            }),
                        },
                        server_info: ServerInfo {
                            name: self.info.name.clone(),
                            version: self.info.version.clone(),
                        },
                    };
                    Some(self.serialize(&result)?)
                }
                "notifications/initialized" => None,
                "tools/list" => {
                    let tools: Vec<ToolInfo> = self
                        .handler
                        .tool_defs()
                        .iter()
                        .map(|t| ToolInfo {
                            name: t.name.to_owned(),
                            description: t.description.to_owned(),
                            input_schema: t.input_schema.clone(),
                        })
                        .collect();
                    let result = ListToolsResult { tools };
                    Some(self.serialize(&result)?)
                }
                "tools/call" => {
                    let result = match &request.params {
                        Some(p) => {
                            match facet_json::from_str::<crate::protocol::CallToolParams>(
                                p.as_ref(),
                            ) {
                                Ok(params) => {
                                    self.handler
                                        .call_tool(&params.name, params.arguments.as_ref())
                                        .await
                                }
                                Err(e) => CallToolResult::error(format!(
                                    "invalid tools/call params: {e}"
                                )),
                            }
                        }
                        None => CallToolResult::error("tools/call requires params"),
                    };
                    Some(self.serialize(&result)?)
                }
                other => {
                    tracing::warn!(method = %other, "unknown MCP method");
                    None
                }
            };

            if let (Some(id), Some(result)) = (request.id, response_result) {
                transport.write_response(id, result).await?;
            }
        }

        Ok(())
    }
}
