# Ship Backend Specification

Backend-specific requirements: ACP integration, crate structure, dependencies,
worktrees, CLI, server configuration, dev proxy, and testability.

## ACP Integration

Ship acts as an ACP client. Each agent (Claude or Codex) is a subprocess that
speaks ACP over stdio. Ship spawns the subprocess, creates a
`ClientSideConnection`, and implements the `Client` trait to handle requests
from the agent.

### Agent Binaries

r[acp.binary.claude]
For Claude agents, Ship MUST support launching either the
`claude-agent-acp` binary directly or, when `npx` is available, the npm
package `@zed-industries/claude-agent-acp` via `npx`. Ship MUST resolve one
concrete launch command and argument list before spawning so discovery and
launch use the same strategy. This agent is a Node.js process that wraps the
Claude Agent SDK as an ACP agent.

r[acp.binary.codex]
For Codex agents, Ship MUST support launching either the `codex-acp` binary
directly or, when `npx` is available, the npm package
`@zed-industries/codex-acp` via `npx`. Ship MUST resolve one concrete launch
command and argument list before spawning so discovery and launch use the
same strategy. This agent wraps codex-rs as an ACP agent.

### Subprocess Lifecycle

r[acp.spawn.stdio]
Ship MUST spawn agent binaries as child processes with piped stdin and stdout
for ACP communication. Stderr MUST be inherited or captured for diagnostics.

r[acp.spawn.kill-on-drop]
Agent child processes MUST be killed when the session is closed or the server
shuts down.

r[acp.spawn.cwd]
Agent child processes MUST be spawned with their working directory set to the
session's git worktree path.

### Connection Setup

r[acp.conn.client-side]
Ship MUST create a `ClientSideConnection` from the `agent-client-protocol`
crate, passing the child process's stdout (read) and stdin (write) streams.

r[acp.conn.local-set]
Because the ACP SDK uses `!Send` futures, the ACP connection's I/O task MUST
be driven on a Tokio `LocalSet` (via `spawn_local`).

r[acp.conn.initialize]
After creating the connection, Ship MUST call `initialize` with its client
info and capabilities before any other ACP method.

r[acp.conn.new-session]
After initialization, Ship MUST call `new_session` with the worktree path
as the working directory and the configured MCP servers.

### MCP Server Configuration

r[acp.mcp.config]
Ship MUST support configuring a list of MCP servers that agents can connect
to. This is specified per session at creation time or globally via server
configuration.

r[acp.mcp.passthrough]
The MCP server list MUST be passed to the agent via `NewSessionRequest`'s
`mcp_servers` field. Ship does not proxy MCP — the agent connects to MCP
servers directly.

r[acp.mcp.defaults]
Ship SHOULD support a default MCP server list (e.g., tracey, filesystem) that
is applied to all sessions unless overridden.

### Client Implementation

Ship implements the ACP `Client` trait to handle requests from agents.

r[acp.client.permission]
Ship MUST implement `request_permission` to surface permission requests to the
UI and block until the human approves or denies.

r[acp.client.session-notification]
Ship MUST implement `session_notification` to receive `SessionUpdate`s from
the agent, including `AgentMessageChunk`, `ToolCall`, `ToolCallUpdate`,
`Plan`, and other update types.

r[acp.client.terminal-create]
Ship MUST implement `create_terminal` to execute commands in the session's
worktree and return a terminal ID.

r[acp.client.terminal-output]
Ship MUST implement `terminal_output` to return the current output and exit
status of a terminal.

r[acp.client.terminal-wait]
Ship MUST implement `wait_for_terminal_exit` to block until a terminal command
completes and return its exit status.

r[acp.client.terminal-kill]
Ship MUST implement `kill_terminal_command` to terminate a running terminal
command.

r[acp.client.terminal-release]
Ship MUST implement `release_terminal` to clean up terminal resources after
the agent is done with them.

r[acp.client.fs-read]
Ship MUST implement `read_text_file` to read files from the session's
worktree.

r[acp.client.fs-write]
Ship MUST implement `write_text_file` to write files to the session's
worktree.

### Prompt Flow

r[acp.prompt.send]
To execute a task, Ship MUST call `prompt` on the `ClientSideConnection` with
the session ID and prompt content.

r[acp.prompt.subscribe]
Ship MUST subscribe to the connection's stream to receive real-time updates
(message chunks, tool calls, plan updates) while the prompt is being processed.

r[acp.prompt.stop-reason]
When the `prompt` call returns, Ship MUST inspect the `PromptResponse`'s stop
reason to determine next steps: `EndTurn` (agent is done), `Cancelled` (prompt
was cancelled), or other reasons.

r[acp.prompt.cancel]
Ship MUST be able to send a `cancel` notification to abort an in-progress
prompt turn.

## Architecture

### Crate Structure

r[crate.ship-types]
A `ship-types` crate MUST define all shared types: identifiers (`SessionId`,
`TaskId`, `BlockId`), enums (`AgentKind`, `Role`, `AgentState`), event types
(`SessionEvent`, `BlockPatch`), error types (`ShipError`), and
request/response structs. This crate MUST NOT depend on any runtime or
framework crates. All types MUST derive `Facet` for serialization over roam.

r[crate.ship-types.error]
The `ShipError` enum MUST be a proper Facet-deriving enum with structured
variants (not `String` errors). Variants MUST include at minimum:
`SessionNotFound`, `ProjectNotFound`, `ProjectInvalid`, `NoActiveTask`,
`AgentError`, `PermissionNotFound`, `WorktreeError`, `InvalidState`. Each
variant carries the relevant context (IDs, messages) as named fields.

r[crate.ship-types.block-id]
`BlockId` MUST be a ULID newtype, like `SessionId` and `TaskId`. It
identifies a content block across its entire lifecycle (creation, patches,
replay). The frontend uses it as the React key and block store index.

r[crate.ship-core]
A `ship-core` crate MUST define the testability traits (`AgentDriver`,
`WorktreeOps`, `SessionStore`) and the core session/task management logic.
This crate depends on `ship-types` and `futures-core`. It MUST NOT depend on
tokio or any specific async runtime. It uses async fn in traits (AFIT) for
the driver traits.

r[crate.ship-service]
A `ship-service` crate MUST define the roam service trait (`Ship`) and its
request/response types. This crate depends on `ship-types` and roam.

r[crate.ship-server]
A `ship-server` crate MUST implement the HTTP server, roam WebSocket endpoint,
Vite dev proxy, and the `Ship` service. This is the binary crate.

### Dependencies

r[dep.axum]
The backend MUST use axum as its HTTP framework.

r[dep.roam]
The backend MUST use roam for the frontend-backend RPC protocol, including
WebSocket transport and TypeScript codegen.

r[dep.facet]
All serializable types MUST derive `Facet` instead of serde traits. Use
facet-based format crates (facet-json, facet-msgpack, etc.) for serialization.

r[dep.tokio]
The backend MUST use Tokio as its async runtime.

r[dep.acp]
The backend MUST use the `agent-client-protocol` crate
(`agentclientprotocol/rust-sdk`) as its ACP client library.

r[dep.ulid]
Session and task identifiers MUST use ULIDs (Universally Unique
Lexicographically Sortable Identifiers) via the `ulid` crate. ULIDs are
sortable by creation time, so session lists and task histories sort correctly
without a separate timestamp field.

r[dep.tracing]
The backend MUST use the `tracing` crate for structured, leveled logging.
Every significant operation (session creation, agent spawn, ACP message
routing, worktree management, permission handling) MUST produce trace spans
and events. This is the diagnostic mechanism for production issues.

r[dep.figue]
The backend MUST use figue for layered configuration parsing. Figue merges
CLI arguments, environment variables, and config files into typed Rust structs
using facet reflection. This replaces ad-hoc `std::env::var` calls with a
single validated config struct at startup.

r[dep.roam-codegen]
The build process MUST use `roam-codegen` to generate TypeScript type
definitions and client stubs from the backend's roam service traits. This is
a build-time tool, not a runtime dependency. The codegen step MUST be
integrated into the build pipeline so generated types stay in sync with Rust
trait changes.

r[dep.reqwest]
The backend MUST use `reqwest` as its async HTTP client for outgoing requests
(Discord webhook POSTs). A single `reqwest::Client` instance MUST be shared
across the server.

### Backend

r[backend.rust]
The backend MUST be implemented in Rust.

r[backend.rpc]
The backend MUST use roam for frontend-backend RPC.

r[backend.agent-lifecycle]
The backend MUST manage agent lifecycle: spawn, connect via ACP, and teardown.

r[backend.session-state]
The backend MUST maintain session state including active task, history, and
agent assignments.

r[backend.message-routing]
The backend MUST translate between the Ship protocol and ACP calls.

r[backend.worktree-management]
The backend MUST manage git worktree creation and cleanup.

r[backend.git-shell]
The `WorktreeOps` implementation MUST shell out to the `git` CLI via
`Command::new("git")` rather than linking against libgit2 (the `git2` crate).
Libgit2 is a large C dependency with build complexity and its worktree support
lags behind the git CLI. The `WorktreeOps` trait keeps this testable — tests
use the in-memory fake, production uses the git CLI.

r[backend.task-persistence]
Task state MUST be persisted to survive server restarts.

r[backend.persistence-format]
Session and task state MUST be stored in a Ship-managed durable store. The
steady-state architecture MUST NOT require per-session JSON files inside
project repositories. The concrete storage engine is intentionally unspecified
so Ship can evolve from the current JSON-backed implementation to a database
or equivalent durable store without changing protocol semantics.

r[backend.persistence.boundary]
The global durable store MUST be the authority for orchestration state that
can span projects or outlive a single checkout, including session records,
cross-project dependency data, shared knowledge, and issue-tracking metadata.
Project-local `.ship/` directories remain authoritative only for repo-scoped
assets such as worktrees and project-local MCP configuration.

r[backend.persistence.migration]
Implementations MAY import legacy session JSON files from project-local
`.ship/` directories during migration, but those files are a compatibility
mechanism, not the long-term storage contract.

r[backend.persistence.links]
Cross-project dependency requests and parent/child session links MUST be
stored as first-class durable records rather than inferred from agent text or
hidden process trees. These records MUST preserve approval state and
coordination-blocked state across restart.

r[backend.persistence.fulfillment]
Dependency request records MUST distinguish child-session task completion from
fulfillment or usable state. Restart recovery MUST preserve whether a human-
gated merge, publish, release, vendoring, or update decision is still pending.

r[backend.persistence-dir-gitignore]
The `.ship/` Ship-owned directory MUST be added to `.gitignore`.

### Server-Side Event Architecture

r[backend.event-log]
The backend MUST maintain an append-only event log per task. Every ACP
notification that translates to a Ship event is appended to this log. The log
is the source of truth for replay — when a new subscriber connects, the
backend re-sends the log entries in order.

r[backend.materialized-state]
The backend MUST also maintain a materialized `SessionState` that represents
the current state of the session: agent states, current task status, block
stores (per role), context levels, pending permissions. This materialized
state is updated synchronously after each event is appended to the log.

r[backend.event-pipeline]
Every ACP notification MUST flow through a single pipeline:
1. Translate the ACP notification to one or more Ship events (BlockAppend,
   BlockPatch, AgentStateChanged, etc.)
2. Append each event to the task's event log with a sequence number
3. Apply each event to the materialized SessionState
4. Broadcast each event to all connected subscribers

This ensures the event log, materialized state, and subscriber streams are
always consistent. There is no separate codepath for "update state" vs
"send events" — they are the same operation.

r[backend.persistence-contents]
The persisted session record in the durable store MUST contain:
- Session config (project, branch, agent kinds, autonomy mode)
- Materialized agent states (for quick `get_session` responses on restart)
- Current task metadata (id, description, status)
- Current task event log (the full list of events, for replay)
- Task history (completed tasks with metadata, no event logs — those are
  discarded when a task completes)

On server restart, the backend loads persisted session records, reconstructs
the materialized state by folding the event log, and resumes. Agent processes
are NOT restored (they are re-spawned via `resilience.agent-crash-recovery`
if a task was in progress).

### Dev Proxy

In development, the Rust backend serves everything on one port. HTTP requests
that don't match API or WebSocket routes are proxied to the Vite dev server.

r[dev-proxy.vite-fallback]
In dev mode, the backend MUST proxy unmatched HTTP requests to the Vite dev
server.

r[dev-proxy.websocket-direct]
The roam WebSocket endpoint MUST be handled directly by the backend, not
proxied through Vite.

r[dev-proxy.vite-lifecycle]
The backend MUST manage the Vite dev server process lifecycle: start it on
launch, wait for TCP readiness, and kill it on shutdown.

r[dev-proxy.vite-port]
The Vite dev server port MUST be configurable via an environment variable
(e.g., `SHIP_VITE_ADDR`, defaulting to `[::]:9141`). The backend passes this
to Vite via `--host` and `--port` flags and uses it as the proxy target.

r[dev-proxy.prod-static]
Production static file serving (built frontend from disk, SPA fallback, cache
headers) is deferred to post-v1. For v1, Ship always runs in dev mode with
the Vite proxy.

## Worktrees

r[worktree.isolated]
Each session MUST operate in an isolated git worktree.

r[worktree.path]
Worktrees MUST be created under `.ship/` relative to the repository root using
an `@{four}` directory name, where `four` is the 4-character lowercase slug
derived from ULID characters 10-13 (the random portion).

r[worktree.gitignore]
The `.ship/` directory entry MUST cover worktree storage; Ship MUST NOT rely on
a separate `.worktrees/` gitignore entry.

r[worktree.base-branch]
Worktrees MUST be created from a user-specified base branch when the session
starts.

r[worktree.branch-name]
Each worktree MUST be created on a new branch named `ship-{four}`, where
`four` is the 4-character lowercase slug derived from ULID characters 10-13
(the random portion).

r[worktree.git-command]
Worktree creation MUST use `git worktree add` with the `--track` flag pointing
to the base branch.

r[worktree.shared]
Both agents in a session MUST operate within the same worktree.

r[worktree.cleanup]
Worktrees MUST be cleaned up on session close, with confirmation from the
human.

r[worktree.cleanup-uncommitted]
If the worktree contains uncommitted changes at cleanup time, the system MUST
warn the human and require explicit confirmation before deleting.

r[worktree.cleanup-git]
Worktree cleanup MUST use `git worktree remove` followed by branch deletion
of the session branch (with `--force` only if the human confirms).

## CLI

r[cli.binary]
The Ship binary MUST be named `ship`. It is the single entry point for all
operations.

r[cli.serve]
`ship serve` MUST start the HTTP server. It does not require being run from
inside a git repository — Ship is not tied to a single repo.

r[cli.open-browser]
On startup, `ship serve` MUST print the URL to stdout. It MUST NOT
auto-open a browser.

r[cli.project-add]
`ship project add <path>` MUST register a git repository as a Ship project.
The path MUST be resolved to an absolute path and validated as a git
repository (contains `.git`). The project is added to Ship's configuration.

r[cli.project-remove]
`ship project remove <name>` MUST unregister a project. Active sessions in
that project MUST be closed first (with confirmation) or the command MUST
fail.

r[cli.project-list]
`ship project list` MUST print all registered projects with their names and
paths.

## Server Configuration

r[server.listen]
The server's HTTP listen address MUST be configurable via the `SHIP_LISTEN`
environment variable, defaulting to `[::1]:9140`.

r[server.multi-repo]
A single Ship server instance manages sessions across all registered
projects. There is no need to run multiple Ship instances.

r[server.mode]
For v1, the server always runs in dev mode (Vite proxy enabled, hot reload).
Production mode is deferred to post-v1 (see `dev-proxy.prod-static`).

r[server.config-dir]
Ship's global configuration MUST be stored in `~/.config/ship/`. This
directory contains `projects.json` (the project registry), optional global
settings, and Ship-managed artifacts for locating or hosting the durable
orchestration store.

r[server.discord-webhook]
The Discord webhook URL MUST be configurable via the `SHIP_DISCORD_WEBHOOK`
environment variable. If unset, Discord notifications are disabled.

r[server.agent-discovery]
On startup, Ship MUST discover agent availability from the same launcher
resolution it uses for spawning. Claude is available when either
`claude-agent-acp` is on `PATH`, or `npx` is on `PATH` and Ship supports
launching `@zed-industries/claude-agent-acp`. Codex is available when either
`codex-acp` is on `PATH`, or `npx` is on `PATH` and Ship supports launching
`@zed-industries/codex-acp`. The availability of each agent kind MUST be
surfaced in the create-session dialog — unavailable agent kinds MUST be
disabled with a `Tooltip` explaining what's missing.

## Testability

The core logic must be testable without spawning real agents, running git
commands, or touching the filesystem. All external interactions go through
traits so tests can substitute in-memory fakes.

r[testability.agent-trait]
Agent communication MUST go through a trait (e.g., `AgentDriver`) that
abstracts ACP connection setup, prompting, cancellation, and notification
streaming. Tests MUST be able to use an in-memory fake that simulates ACP
responses without spawning real agent processes.

r[testability.git-trait]
Git worktree operations MUST go through a trait (e.g., `WorktreeOps`) that
abstracts worktree creation, cleanup, branch management, and uncommitted-change
detection. Tests MUST be able to use an in-memory fake.

r[testability.persistence-trait]
Task and session persistence MUST go through a trait (e.g., `SessionStore`)
that abstracts reading, writing, and listing persisted state. Tests MUST be
able to use an in-memory store.

r[testability.no-subprocess-in-tests]
Unit and integration tests MUST NOT spawn real agent binaries or require
external processes. All tests that exercise session management, task lifecycle,
steer flow, permission handling, or event streaming MUST run against trait
fakes.

r[testability.trait-location]
Testability traits (`AgentDriver`, `WorktreeOps`, `SessionStore`) MUST be
defined in a `ship-core` crate, not in `ship-types`. The `ship-core` crate
depends on `ship-types` for the shared types and is allowed to depend on
`futures-core` (for `Stream`) and use Rust async fn in traits (AFIT). It
MUST NOT depend on tokio or any specific runtime. The `ship-server` crate
depends on `ship-core` and provides the real implementations.
