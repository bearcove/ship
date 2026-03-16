mod protocol;
mod server;
mod transport;

pub use facet_json_schema::{JsonSchema, schema_for};
pub use protocol::*;
pub use server::{McpServer, McpServerInfo, ServerError, ToolDef, ToolHandler, ToolResult};
pub use transport::{StdioTransport, TransportError};
