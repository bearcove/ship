# `kagi_web_search` and MCP utility functions are copy-pasted between captain and mate servers

`crates/ship-server/src/captain_mcp_server.rs` and `crates/ship-server/src/mate_mcp_server.rs` both contain identical copies of:

- `kagi_web_search` (~65 lines) — verbatim duplicate, does Kagi FastGPT API call and formats references
- `tool_result` (8 lines) — builds a `CallToolResult` from text + is_error flag
- `metadata_string` (7 lines) — builds a borrowed `MetadataEntry`
- `metadata_string_owned` (7 lines) — builds an owned `MetadataEntry`

The only differences are the `call_tool_rpc_error` message string (`"captain MCP RPC failed"` vs `"mate MCP RPC failed"`), and mate has an extra `mcp_tool_call_result` helper for responses with diffs.

## Fix

Extract the shared functions into `crates/ship-server/src/mcp_common.rs` (or extend `worktree_tools.rs` which already exists for shared tool definitions). Import from both servers.
