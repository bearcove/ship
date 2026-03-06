#[path = "captain_mcp_server.rs"]
mod server;

pub use server::{CaptainMcpServerArgs, run_stdio_server};
