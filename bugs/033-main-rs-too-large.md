# `main.rs` is 1289 lines with unrelated concerns mixed together

`crates/ship-server/src/main.rs` contains at least four distinct concerns that make it hard to navigate:

- **CLI parsing + dispatch** — `Args`, `Command`, `ServeArgs`, `ProjectCommand`, etc.
- **HTTP server setup** — axum router, WebSocket handler, static frontend serving
- **Vite dev server management** — spawning Vite, waiting for readiness, HMR host resolution (~100 lines, lines ~695–810)
- **Vite WebSocket proxy** — bidirectional WS proxy between browser and Vite dev server (~300 lines, lines ~858–1173)
- **Project utility commands** — `project_add`, `project_list`, `project_remove` (~50 lines)

The Vite proxy code in particular (`proxy_vite_handler`, `handle_vite_ws_upgrade`, `proxy_vite_ws`, `run_ws_proxy`, header filter functions) is a self-contained subsystem buried at the bottom of `main.rs`.

## Fix

Extract into focused modules:
- `vite_proxy.rs` — all Vite dev server + WebSocket proxy logic
- `project_commands.rs` — `project_add/list/remove`
- `serve.rs` — HTTP server setup and axum router
Keep `main.rs` as a thin dispatcher that delegates to these.
