# Ship

Pair programming with AI agents. A captain steers, a mate builds.

## What it is

Ship coordinates two AI coding agents—captain and mate—working together on a shared codebase. The captain reviews, directs, and assigns tasks. The mate writes code, runs tests, and implements. The human watches, steers the captain, and approves sensitive actions.

Each session runs in an isolated git worktree on its own branch. Sessions are persistent across restarts.

## How it works

```
Browser (React)
    |
    | roam WebSocket (generated TypeScript types)
    |
    v
Backend (Rust)
    |
    |--- ACP ---> Claude Code (captain)
    |--- ACP ---> Claude Code / Codex (mate)
    |--- git ---> worktree per session
    |--- fs  ---> task persistence
```

**ACP** (Agent Client Protocol) gives the backend structured access to agent state: prompts, notifications, stop reasons, plan steps, content blocks, and permission requests. No terminal scraping, no polling.

## Agent roles

### Captain

The captain gets a restricted MCP toolset — no shell access, read-only file access:

- `captain_assign` — assign a task to the mate (title + description)
- `captain_steer` — send direction to the mate mid-task
- `captain_accept` — accept the mate's work and close the task
- `captain_cancel` — cancel the current task
- `captain_notify_human` — block until the human responds
- `captain_read_file`, `captain_search_files`, `captain_list_files` — read-only codebase access

### Mate

The mate gets full implementation tools:

- `run_command` — shell command in the worktree
- `read_file`, `write_file` — file access (Rust files are auto-formatted)
- `edit_prepare` / `edit_confirm` — two-step search-and-replace with diff preview
- `search_files`, `list_files` — ripgrep and fd
- `cargo_check`, `cargo_clippy`, `cargo_test` — Rust toolchain
- `pnpm_install` — frontend dependency management
- `set_plan` — declare a work plan (required before any file changes)
- `plan_step_complete` — mark a step done and commit its changes
- `mate_send_update` — non-blocking progress update to the captain
- `mate_ask_captain` — blocking question to the captain
- `mate_submit` — submit work for captain review

## Task lifecycle

```
assign → Working → [mate produces updates] → steer
                                                ↓
                                            accept / cancel
```

1. Human or captain sends `assign` with a task title and description
2. Backend prompts the mate via ACP
3. Mate works—plan steps, tool calls, and text stream back in real time
4. When the mate submits, the captain reviews and accepts, steers, or cancels
5. Accepted tasks move to history; the session is ready for the next task

## UI features

- **Session list** — all sessions with agent states, current task, branch name, auto-generated title
- **Session view** — unified feed of captain and mate output side by side
- **Unified feed** — interleaved `BlockAppend` / `BlockPatch` events from both agents; content blocks include text, tool calls (with diffs and terminal output), plan updates, and permission requests
- **Composer** — steers the captain by default; prefix with `@mate` to steer the mate directly; `@` also triggers file mention autocomplete from the worktree; images can be attached by paste or drag-and-drop
- **Permission approval** — inline approve/deny UI when an agent requests a sensitive action; options include allow-once, allow-always, reject-once, reject-always
- **Agent state** — Working (with optional plan and current activity), Idle, AwaitingPermission, ContextExhausted, Error
- **Plan steps** — displayed with status (Pending / InProgress / Completed / Failed) and priority
- **Context warning** — chips appear when context falls below 20%
- **Auto-title** — session title generated in the background after the first message
- **Scroll-to-bottom button** — appears when scrolled up in a live feed
- **Agent discovery** — detects whether Claude Code and Codex are installed on the host

## Architecture

- **Backend**: Rust, async Tokio. `ship-server` serves the roam WebSocket endpoint and hosts the captain and mate MCP servers. `ship-core` manages session lifecycle, ACP connections, and git worktrees. `ship-types` defines all shared types (Facet-derived, no serde).
- **Frontend**: React + Radix Themes + Vanilla Extract. TypeScript bindings are generated from the Rust service trait via `cargo xtask codegen`.
- **RPC**: [roam](https://github.com/bearcove/roam) — Rust traits as the schema, bidirectional streaming over WebSocket.
- **Worktrees**: Created under `.ship/@{id}/` on a `ship-{id}` branch when a session starts.

## Developing

Default server mode serves the built frontend from `frontend/dist`:

```sh
cd frontend
pnpm build
cd ..
just run
# or: cargo run --bin ship -- serve
```

Dev mode starts Vite behind the backend and preserves the proxy + HMR workflow:

```sh
cargo run --bin ship -- serve --dev
```

Typecheck:

```sh
pnpm exec tsgo --noEmit
```

Lint / format:

```sh
pnpm exec oxlint frontend/src/
pnpm exec oxfmt --write frontend/src/
```

Regenerate TypeScript bindings after changing `ship-service`:

```sh
cargo xtask codegen
```
