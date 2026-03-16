mod context;
mod protocol;
mod server;
mod tool_macro;
mod transport;

pub use context::ToolCtx;
pub use facet_json_schema::{JsonSchema, schema_for};
pub use protocol::*;
pub use server::{McpServer, McpServerInfo, ServerError, Tool, ToolError, ToolResult};
pub use transport::{StdioTransport, TransportError};
