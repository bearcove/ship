# `ship_impl.rs` is a 7097-line monolith

`crates/ship-server/src/ship_impl.rs` is 7097 lines with 185 functions. It contains at least six distinct logical layers that belong in separate modules:

| Lines | Content |
|---|---|
| 1–188 | Supporting structs/enums (`PreparedEdit`, `PendingMcpOps`, etc.) |
| 189–1297 | `ShipImpl` core: session startup, MCP install, subscriptions |
| 1298–2084 | Captain tool implementations (`captain_tool_assign/steer/accept/cancel/notify_human/read_file`) |
| 2085–3063 | Mate file tool implementations (`run_command`, `read_file`, `write_file`, `edit_prepare`, `edit_confirm`) |
| 3064–3600 | Mate workflow tools (`send_update`, `set_plan`, `plan_step_complete`, `ask_captain`, `submit`) |
| 3601–4490 | Session lifecycle: `start_session_runtime`, `prompt_agent`, `drain_notifications`, `persist_session`, guardrails |
| 4491–5519 | `impl Ship for ShipImpl` (RPC surface, 1029 lines) |
| 5520–5856 | `CaptainMcpSessionService` + `MateMcpSessionService` impls |
| 5857–7097 | Tests |

## Fix

Convert `ship_impl.rs` into a `ship_impl/` module directory. Natural submodules:
- `captain_tools.rs` — `captain_tool_*` methods
- `mate_file_tools.rs` — `mate_tool_run_command`, `read_file`, `write_file`, `edit_prepare`, `edit_confirm` and their helpers (rustfmt, sandboxed_sh, path validation, etc.)
- `mate_workflow_tools.rs` — `mate_tool_send_update`, `set_plan`, `plan_step_complete`, `ask_captain`, `submit`
- `session_lifecycle.rs` — startup, `prompt_agent`, `drain_notifications`, `persist_session`, guardrails
- `mod.rs` — `ShipImpl` struct + `impl Ship for ShipImpl` surface
