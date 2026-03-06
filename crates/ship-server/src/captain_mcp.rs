#[path = "captain_mcp_proxy.rs"]
mod proxy;
#[path = "captain_mcp_server.rs"]
mod server;

pub use proxy::run_proxy;
pub use server::{CaptainMcpServerHandle, ToolDefinition, ToolHandler, ToolResult, start_server};
