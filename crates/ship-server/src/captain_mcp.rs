#[path = "admiral_mcp_server.rs"]
mod admiral_server;
#[path = "captain_mcp_server.rs"]
mod captain_server;
#[path = "mate_mcp_server.rs"]
mod mate_server;

pub use admiral_server::{AdmiralMcpServerArgs, run_stdio_server as run_admiral_stdio_server};
pub use captain_server::{CaptainMcpServerArgs, run_stdio_server as run_captain_stdio_server};
pub use mate_server::{MateMcpServerArgs, run_stdio_server as run_mate_stdio_server};
