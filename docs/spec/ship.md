# Ship Specification

Pair programming with AI agents. A captain steers, a mate builds.

Ship coordinates two AI coding agents working together on a shared codebase.
One plays captain (architecture, review, direction), one plays mate (writes
code, runs tests, implements). The human watches, intervenes when needed, and
approves actions.

Built on ACP (Agent Client Protocol) for direct structured communication with
Claude Code and OpenAI Codex. Web-based UI for visibility and control.

## Sessions

A session is a pairing: one captain agent and one mate agent collaborating on
a branch.

r[session.create]
The system MUST allow creating a session with a specified project, captain
agent kind, mate agent kind, base branch, and initial task description.
Session creation and first task assignment are a single atomic operation —
there is no session without a task.

r[session.list]
The system MUST allow listing all active sessions with their current state.

r[session.persistent]
Sessions MUST be persistent across browser reloads and server restarts.
Agents continue running after the browser tab is closed. When the human
returns, the session list shows updated state and the hydration flow
(per `proto.hydration-flow`) restores full context.

r[session.single-task]
Each session MUST have at most one active task at a time, plus a history of
completed tasks.

### Agent Assignment

r[session.agent.captain]
Each session MUST have exactly one captain agent (Claude or Codex, user's
choice).

r[session.agent.mate]
Each session MUST have exactly one mate agent (Claude or Codex, user's choice).

r[session.agent.kind]
The system MUST support at least two agent kinds: Claude and Codex.

## Agent Communication

Agents are controlled via ACP (Agent Client Protocol).

r[acp.prompt]
The backend MUST send structured instructions to agents via ACP
`SessionPrompt`.

r[acp.notifications]
The backend MUST receive agent state changes via ACP notifications.

r[acp.stop-reason]
The backend MUST use ACP stop reasons to determine why an agent paused (done,
tool use, token limit), not heuristics.

r[acp.plans]
The backend MUST surface the agent's execution plan with per-step status when
available.

r[acp.content-blocks]
The backend MUST relay agent content blocks (text, tool calls, images, diffs)
as typed data.

r[acp.permissions]
The backend MUST surface agent permission requests to the UI and relay
human decisions back to the agent.

r[acp.terminals]
The backend MUST support managed command execution with exit codes through ACP
terminal facilities.

## ACP Integration

Ship acts as an ACP client. Each agent (Claude or Codex) is a subprocess that
speaks ACP over stdio. Ship spawns the subprocess, creates a
`ClientSideConnection`, and implements the `Client` trait to handle requests
from the agent.

### Agent Binaries

r[acp.binary.claude]
For Claude agents, Ship MUST spawn the `claude-agent-acp` binary
(`@zed-industries/claude-agent-acp` npm package). This is a Node.js process
that wraps the Claude Agent SDK as an ACP agent.

r[acp.binary.codex]
For Codex agents, Ship MUST spawn the `codex-acp` binary
(`zed-industries/codex-acp` Rust crate). This is a native process that wraps
codex-rs as an ACP agent.

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
the agent, including `AgentMessageChunk`, `ToolCallStart`, `ToolCallDone`,
`PlanUpdate`, and other update types.

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
Session and task state MUST be serialized using facet-json and stored as JSON
files in a `.ship/` directory relative to the session's project repository
root. Each session gets a `{session_id}.json` file containing the session
config, current task, and task history.

r[backend.persistence-dir-gitignore]
The `.ship/` persistence directory MUST be added to `.gitignore`.

### Frontend

r[frontend.typescript]
The frontend MUST be implemented in TypeScript with strict mode enabled.

r[frontend.react]
The frontend MUST use React 19 for UI rendering.

r[frontend.vite]
The frontend MUST use Vite 6 as its dev server and build tool, with the
`@vitejs/plugin-react` plugin.

r[frontend.codegen]
Frontend types MUST be generated from the backend's Rust traits via
roam-codegen.

#### Styling

r[frontend.style.vanilla-extract]
The frontend MUST use vanilla-extract for styling. All styles MUST be defined
in `.css.ts` files as typed TypeScript, producing zero-runtime static CSS at
build time.

r[frontend.style.vite-plugin]
The Vite config MUST include the `@vanilla-extract/vite-plugin` for build-time
CSS extraction.

#### Component Library

r[frontend.components.radix]
The frontend MUST use Radix Themes (`@radix-ui/themes`) as its pre-styled
component library for buttons, dialogs, dropdowns, badges, cards, tabs, and
layout primitives.

r[frontend.components.radix-theme]
The Radix Theme provider MUST be configured at the app root with a dark
appearance and accent color suitable for a developer tool.

r[frontend.components.radix-overrides]
Where Radix Themes defaults are insufficient, overrides MUST be applied via
vanilla-extract using Radix's CSS custom properties, not by fighting the
component internals.

#### Routing

r[frontend.routing]
The frontend MUST use react-router-dom v7 for client-side routing between the
session list view and individual session views.

#### Icons

r[frontend.icons]
The frontend MUST use `@phosphor-icons/react` for iconography.

#### Testing

r[frontend.test.vitest]
Frontend tests MUST use vitest as the test runner.

r[frontend.test.rtl]
Frontend component tests MUST use `@testing-library/react` for rendering and
assertions.

#### Package Structure

r[frontend.package.private]
The frontend MUST be a private npm package (`"private": true`) in a `frontend/`
directory at the repository root.

r[frontend.package.type-module]
The frontend package MUST use `"type": "module"` for ES module support.

r[frontend.package.scripts]
The frontend package MUST define at minimum these scripts: `dev` (vite dev
server), `build` (typecheck + vite build), `typecheck` (tsc --noEmit), and
`test` (vitest run).

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

## Views

### Session List

r[view.session-list]
The UI MUST display a session list showing all sessions, their agent states,
and current tasks at a glance.

### Session View

r[view.session]
The UI MUST display a session view with captain and mate panels side by side,
plus task controls.

### Agent Panel

r[view.agent-panel.state]
Each agent panel MUST display the agent's current state (working, idle,
awaiting permission, context exhausted).

r[view.agent-panel.context]
Each agent panel MUST display context window usage when available.

r[view.agent-panel.plan]
Each agent panel MUST display the agent's execution plan when available.

r[view.agent-panel.activity]
Each agent panel MUST display current activity description when the agent is
working.

### Task Panel

r[view.task-panel]
The UI MUST display a task panel with the active task description, update
history, and steer/accept/cancel controls.

### Permission Dialog

r[view.permission-dialog]
The UI MUST display permission requests inline and allow the human to approve
or deny agent actions.

### Content Rendering

r[view.no-terminal]
The UI MUST NOT include a terminal emulator. Content blocks (code, diffs, text)
MUST be rendered directly as structured elements.

## Protocol

The Ship RPC defines frontend-to-backend operations.

### Identifiers

r[proto.id.session]
Session identifiers MUST be ULIDs wrapped in a `SessionId` newtype.

r[proto.id.task]
Task identifiers MUST be ULIDs wrapped in a `TaskId` newtype.

### Operations

r[proto.id.project]
Project identifiers MUST be the project name string (unique, derived from
directory name at registration time).

r[proto.create-session]
The protocol MUST support a `create_session` operation that takes a project
name, agent configuration, base branch, and an initial task description. It
creates the session, worktree in the project's repo, spawns agents, and
assigns the first task in one call. Returns a `SessionId` and `TaskId`.

r[proto.list-projects]
The protocol MUST support a `list_projects` operation that returns all
registered projects with their names, paths, and validation status.

r[proto.add-project]
The protocol MUST support an `add_project` operation that registers a new
project by path. This is the RPC equivalent of `ship project add`.

r[proto.remove-project]
Project removal is CLI-only (`ship project remove`). There is no RPC
operation for removing projects from the UI. This prevents accidental removal
while sessions are active.

r[proto.list-branches]
The protocol MUST support a `list_branches` operation that takes a project
name and returns its git branches (local and remote-tracking). The frontend
uses this to populate the branch selector in the create-session dialog.

r[proto.list-sessions]
The protocol MUST support a `list_sessions` operation that returns summaries
of all active sessions.

r[proto.assign]
The protocol MUST support an `assign` operation where the human assigns a new
task to the session, returning a `TaskId`. On assignment, the backend first
prompts the captain for direction (per `captain.initial-prompt`), then prompts
the mate with the task description plus the captain's direction.

r[proto.steer]
The protocol MUST support a `steer` operation where the captain provides
feedback or new direction to the mate.

r[proto.accept]
The protocol MUST support an `accept` operation where the captain accepts the
mate's work and closes the task.

r[proto.cancel]
The protocol MUST support a `cancel` operation that cancels the current task.

r[proto.resolve-permission]
The protocol MUST support a `resolve_permission` operation to respond to agent
permission requests.

r[proto.retry-agent]
The protocol MUST support a `retry_agent` operation that takes a session ID
and a role (captain or mate). It respawns the agent process, reinitializes
the ACP connection, and if a task was in progress, triggers crash recovery
(per `resilience.agent-crash-recovery`).

r[proto.close-session]
The protocol MUST support a `close_session` operation that tears down both
agents, triggers worktree cleanup (with confirmation), and removes the session
from the active list. The session's persistence file is retained for history.

r[proto.get-session]
The protocol MUST support a `get_session` operation that returns the session's
structural state: agent snapshots, current task metadata and status, task
history summaries, autonomy mode, and control status. This provides the
skeleton; content blocks come via event replay.

r[proto.hydration-flow]
Frontend hydration MUST follow this sequence: first call `get_session` for
structural state, then open the event subscription channel. The channel
replays the current task's content block history (per
`event.subscribe.replay`) before switching to live events. The two mechanisms
are complementary, not alternatives.

## Agent State

r[agent-state.derived]
Agent state MUST be derived from ACP events, not from inference or heuristics.

### States

r[agent-state.working]
The `Working` state MUST include an optional execution plan and an optional
activity description.

r[agent-state.idle]
The `Idle` state indicates the agent has finished and is waiting for input.

r[agent-state.awaiting-permission]
The `AwaitingPermission` state MUST include the pending permission request.

r[agent-state.context-exhausted]
The `ContextExhausted` state indicates the agent has hit its context window
limit.

r[agent-state.error]
The `Error` state indicates the agent has failed. It MUST include an error
message describing what went wrong (spawn failure, ACP protocol error, crash,
etc.). An agent in the `Error` state cannot receive prompts until it is
restarted.

### Error Conditions

r[error.spawn-failure]
If an agent binary fails to spawn (not found, permission denied, npm not
installed, etc.), the agent MUST transition to the `Error` state with a
descriptive message and the session MUST remain usable for the other agent.

r[error.acp-init-failure]
If ACP `initialize` or `new_session` fails, the agent MUST transition to the
`Error` state. The backend MUST NOT retry automatically — the human decides
whether to retry or reconfigure.

r[error.agent-crash]
If an agent process exits unexpectedly (non-zero exit, signal), the agent MUST
transition to the `Error` state. The backend MUST capture stderr output and
include it in the error message.

r[error.frontend-display]
The frontend MUST display agent error states prominently, including the error
message. It MUST offer a "retry" action that attempts to respawn and
reinitialize the agent.

### Plan Steps

r[agent-state.plan-step]
Each plan step MUST have a description and a status (planned, in-progress,
completed, or failed).

### Snapshot

r[agent-state.snapshot]
An `AgentSnapshot` MUST include the agent's role, kind, state, and an optional
context remaining percentage (0-100).

## Worktrees

r[worktree.isolated]
Each session MUST operate in an isolated git worktree.

r[worktree.path]
Worktrees MUST be created under `.worktrees/ship-{session_id}/` relative to
the repository root.

r[worktree.gitignore]
The `.worktrees/` directory MUST be added to the repository's `.gitignore`.

r[worktree.base-branch]
Worktrees MUST be created from a user-specified base branch when the session
starts.

r[worktree.branch-name]
Each worktree MUST be created on a new branch named
`ship/{session_short_id}/{slug}` where `session_short_id` is the first 8
characters of the session ULID and `slug` is a kebab-case summary derived from
the initial task description (always available since session creation requires
a task).

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

## Task Lifecycle

### Task Status

r[task.status.enum]
A task's status MUST be one of the following values:

- `Assigned` — task has been created, captain is being prompted for direction
- `Working` — the mate is actively processing a prompt
- `ReviewPending` — the mate has finished and the captain is reviewing
- `SteerPending` — the captain has produced a steer awaiting human approval
  (human-in-the-loop mode only)
- `Accepted` — the task is complete, work is done
- `Cancelled` — the task was cancelled by the human or captain

r[task.status.terminal]
`Accepted` and `Cancelled` are terminal states. Once a task reaches a terminal
state, it moves to the session's task history and cannot be modified.

### Task Flow

r[task.assign]
Task creation begins when the human (or captain) sends an `assign` with a task
description.

r[task.prompt]
On assignment, the backend MUST send a `SessionPrompt` to the mate via ACP.

r[task.progress]
While the mate works, the backend MUST receive ACP notifications and stream
progress to the frontend in real time.

r[task.completion]
When the mate finishes (`StopReason::EndTurn`), the captain reviews the output.

r[task.steer]
The captain MUST be able to send `steer` to request more work from the mate.

r[task.accept]
On accept, the task MUST move to history and the session MUST be ready for the
next task.

r[task.cancel]
Tasks MUST be cancellable at any point in their lifecycle.

## Event Stream

r[event.subscribe]
The frontend MUST be able to subscribe to a session's event stream.

r[event.subscribe.roam-channel]
Event subscription MUST be implemented as a roam bidirectional channel. The
frontend opens the channel by session ID and receives `SessionEvent`s as a
typed stream. The channel is part of the `Ship` service trait.

r[event.subscribe.replay]
When a new subscriber connects, the backend MUST replay the current task's
content block history before streaming live events. This ensures late-joining
browsers see the full current state without a separate hydration call. Replay
events use the same `SessionEvent` types as live events — the frontend does
not distinguish between replayed and live events.

### Event Envelope

r[event.envelope]
Every event sent to the frontend MUST be wrapped in a `SessionEvent` envelope
containing: a monotonically increasing sequence number (`seq`), the originating
agent role, and the event payload. The sequence number is per-session and
starts at 0 for each new task. Replay events use the original sequence numbers.

r[event.ordering]
Events MUST be delivered in sequence order. If the frontend receives an event
with a sequence gap (missed events), it MUST re-subscribe to get a full replay.
The backend MUST NOT send events out of order.

### Content Blocks and Stable IDs

r[event.block-id]
Every content block MUST have a server-assigned stable ID (`BlockId`), which
is a ULID. The backend assigns the ID when it first creates the block from an
ACP notification. The same block ID is used in all subsequent updates to that
block. The frontend uses this ID as the React key and as the index into its
block store.

r[event.block-id.tool-call]
For tool calls, the `BlockId` MUST be derived deterministically from the ACP
`ToolCallId` (e.g., by hashing or namespacing). This ensures that the initial
`ToolCall` notification and all subsequent `ToolCallUpdate` notifications map
to the same Ship block.

r[event.block-id.text]
For agent text, the backend MUST accumulate consecutive `AgentMessageChunk`
notifications from the same prompt turn into a single text block. The block ID
is assigned on the first chunk and reused for all subsequent chunks in the same
turn. A new prompt turn starts a new text block with a new ID.

r[event.block-id.plan]
Plan updates are full replacements (ACP sends the entire plan each time). Each
agent has at most one active plan block. The block ID for the plan is assigned
on the first `Plan` notification and reused for all subsequent plan updates
from the same agent within the same task.

r[event.block-id.permission]
Permission request blocks MUST use a block ID derived from the ACP
`ToolCallId` in the `RequestPermissionRequest`. This links the permission
visually to the tool call that triggered it.

### Event Types

r[event.append]
A `BlockAppend` event MUST be emitted when a new content block is created. It
contains the block ID, the role, and the full initial block data. The frontend
MUST insert this block at the end of the role's block list.

r[event.patch]
A `BlockPatch` event MUST be emitted when an existing block is updated. It
contains the block ID, the role, and a patch payload describing what changed.
The frontend MUST find the block by ID and apply the patch in place.

The following patch variants MUST be supported:

r[event.patch.text-append]
`TextAppend` — appends text to an existing text block. Used when additional
`AgentMessageChunk` notifications arrive for the same turn.

r[event.patch.tool-call-update]
`ToolCallUpdate` — updates a tool call block's status, result, and/or output.
Used when ACP sends a `ToolCallUpdate` notification. The patch includes the
new status (`running`, `success`, `failure`), optional result text, and
optional output.

r[event.patch.plan-replace]
`PlanReplace` — replaces the entire plan step list. Used when ACP sends a new
`Plan` notification. Plans are always full replacements, never deltas.

r[event.patch.permission-resolve]
`PermissionResolve` — updates a permission block with the human's decision
(`approved` or `denied`). Emitted when `resolve_permission` is called.

r[event.agent-state-changed]
The system MUST emit `AgentStateChanged` events when an agent's state changes,
including the role and new state. This is a top-level event, not a block
append/patch.

r[event.task-status-changed]
The system MUST emit `TaskStatusChanged` events when a task's status changes,
including the task ID and new status. This is a top-level event.

r[event.context-updated]
The system MUST emit `ContextUpdated` events when an agent's context usage
changes, including the role and remaining percentage. This is a top-level
event.

### Content Block Types

r[event.content-block.types]
Ship MUST support the following content block types, mapped from ACP
`SessionUpdate` / `SessionNotification` types:

- `Text` — accumulated agent message text. Multiple `AgentMessageChunk`
  notifications from the same prompt turn are merged into one block. Has a
  `text: String` field that grows via `TextAppend` patches.
- `ToolCall` — a single tool invocation lifecycle. Created from ACP `ToolCall`
  notification, updated via `ToolCallUpdate` patches. Fields: `tool_call_id`,
  `tool_name`, `status` (pending/running/success/failure), `arguments`,
  optional `result`, optional `output`.
- `Plan` — the agent's execution plan. Created on first ACP `Plan`
  notification, replaced in full on subsequent ones. Fields: `steps: Vec<PlanStep>`
  where each step has `description`, `status` (pending/in-progress/completed/failed),
  and `priority` (high/medium/low).
- `Error` — an error message. Created when the backend detects an agent error
  condition. Fields: `message: String`.
- `Permission` — a permission request from an agent. Created from ACP
  `RequestPermissionRequest`. Fields: `tool_call_id`, `tool_name`,
  `description`, `arguments`, `options: Vec<PermissionOption>`,
  optional `resolution`.

The frontend MUST have a renderer for each block type.

### Frontend Block Store

r[event.store.structure]
The frontend MUST maintain a per-role ordered block store. The store is a list
of `(BlockId, Block)` pairs, preserving insertion order. A separate index
(map from `BlockId` to list position) MUST be maintained for O(1) patch
lookups.

r[event.store.append]
On `BlockAppend`, the frontend MUST append the block to the end of the
role's list and add it to the index.

r[event.store.patch]
On `BlockPatch`, the frontend MUST look up the block by ID, apply the patch
to produce a new block value, and trigger a re-render of that block only.
If the block ID is not found (e.g., due to a missed event), the frontend
MUST re-subscribe to get a full replay.

r[event.store.clear-on-new-task]
When a new task is assigned, the backend MUST emit a `TaskStarted` event
(a top-level event, not a block event). On receiving this, the frontend MUST
clear both block stores and the sequence counter. The new task's events start
from sequence 0.

r[event.store.immutable-updates]
Block store updates MUST use immutable data patterns (new object references
on mutation) so that React can detect changes via reference equality. Do NOT
mutate block objects in place.

### Replay Semantics

r[event.replay.same-events]
Replay MUST send the same `BlockAppend` and `BlockPatch` events that were
originally emitted, in order. The frontend processes them identically to
live events. There is no special "replay mode" — the store is built from
events regardless of whether they are replayed or live.

r[event.replay.snapshot-optimization]
As a future optimization, the backend MAY send a single `Snapshot` event
containing the full block store state instead of replaying individual events.
This is not required for v1.

r[event.replay.marker]
The backend MUST send a `ReplayComplete` top-level event after all replay
events have been sent. This allows the frontend to distinguish between
"still loading history" and "caught up to live". The frontend MAY show a
loading indicator until `ReplayComplete` is received.

## Resilience

r[resilience.state-in-backend]
Task state MUST live in the backend, not in the agent, so nothing is lost on
disconnection.

r[resilience.agent-crash-recovery]
If an agent process crashes, the backend MUST respawn the agent, reinitialize
the ACP connection, create a new session, and inject a summary prompt that
describes the task, what the agent had done so far, and the current state of
the worktree. The agent resumes from this summary, not from conversation
history.

r[resilience.load-session-future]
ACP's `LoadSession` capability (resuming an existing agent session with full
conversation history) is a future enhancement. For v1, crash recovery uses
respawn + summary prompt as described above.

r[resilience.server-restart]
When Ship restarts, all agent processes are dead (they were children of the
previous server process). On startup, Ship MUST load persisted session state
from each project's `.ship/` directory. Sessions with non-terminal tasks
(status is not `Accepted` or `Cancelled`) MUST be displayed in the session
list with both agents in the `Error` state and a message indicating "Server
restarted — agents need respawn." The human can then click "Retry" on each
agent to respawn and trigger crash recovery (per
`resilience.agent-crash-recovery`). Ship MUST NOT auto-respawn agents on
restart — the human decides which sessions to resume.

## Session Sharing

r[sharing.multi-browser]
Multiple browsers MUST be able to watch the same session simultaneously.

r[sharing.event-broadcast]
Every connected client MUST receive the same `SessionEvent`s via roam's
multi-subscriber support.

r[sharing.single-writer]
Steering MUST be single-writer: one active controller per session at a time.

r[sharing.claim-control]
A browser MUST be able to claim control of a session via a "take control"
action. The first browser to connect to a session automatically becomes the
controller.

r[sharing.release-control]
When the controlling browser disconnects (WebSocket close, tab close, crash),
control MUST be released automatically. Any other connected browser can then
claim it.

r[sharing.viewer-ui]
Non-controlling browsers MUST see the session in read-only mode: the event
stream, agent panels, and content blocks are visible, but steer/accept/cancel
controls and permission approval buttons MUST be disabled or hidden.

## Captain Role

The captain is an AI agent whose job is architecture, review, and direction.
It does not write code directly — it steers the mate.

### Captain Prompting

r[captain.system-prompt]
The captain's system prompt MUST instruct it to act as a senior engineer
reviewing the mate's work: set direction, decompose tasks, review output,
and formulate steer instructions. It MUST NOT instruct the captain to write
code directly.

r[captain.initial-prompt]
When a task is assigned, the backend MUST prompt the captain with the task
description. The captain's response is the initial direction sent to the mate
as part of the assignment prompt.

r[captain.context]
When the mate finishes a prompt turn (`StopReason::EndTurn`), the backend MUST
collect the mate's output (text, tool call summaries, file diffs) and inject
it into the captain's next prompt as structured context. The captain sees what
the mate did, not raw ACP payloads.

r[captain.review-trigger]
The backend MUST automatically prompt the captain for review whenever the mate
finishes a prompt turn. This happens regardless of autonomy mode — the
difference is whether the captain's steer requires human approval before
reaching the mate.

r[captain.prompt-template]
The captain's review prompt MUST follow a template that includes: the original
task description, the steer history so far, and the mate's latest output. The
prompt MUST ask the captain to either provide a steer (more work needed) or
signal acceptance (task complete).

r[captain.no-filesystem]
The captain agent MUST NOT be configured with filesystem or terminal ACP
capabilities. It reviews output, it does not execute. The captain does have
access to Ship's MCP tools (`ship_steer`, `ship_accept`, `ship_reject`) —
these are MCP tools, not ACP filesystem/terminal capabilities.

### Captain Tools

The captain communicates decisions to Ship via ACP extension tools, exposed
as MCP tools on the captain's session. This avoids fragile text parsing.

r[captain.tool.steer]
The captain MUST have access to a `ship_steer` tool that takes a `message`
argument (string). When the captain calls this tool, the backend interprets
the message as a steer instruction for the mate.

r[captain.tool.accept]
The captain MUST have access to a `ship_accept` tool that takes an optional
`summary` argument (string). When the captain calls this tool, the backend
transitions the task to accepted.

r[captain.tool.reject]
The captain MUST have access to a `ship_reject` tool that takes a `reason`
argument (string) and a `message` argument (string). Rejection means the
captain believes the current approach is fundamentally wrong. The backend
cancels the mate's in-progress prompt (if any) via ACP `cancel`, transitions
the task to `Cancelled` with the reason, and surfaces the captain's message
to the human. The human can then assign a new task with a different approach.
This is the same codepath as `proto.cancel` — the only difference is the
initiator (captain vs human) and that the captain provides a reason.

r[captain.tool.implementation]
Captain tools MUST be implemented as MCP tools served by Ship itself. The
captain's `NewSessionRequest` MUST include Ship's MCP server in its
`mcp_servers` list so the captain can discover and call these tools.

r[captain.tool.transport]
Ship MUST expose its captain MCP tools via a per-captain stdio proxy. For each
captain agent, Ship spawns a pair of connected pipes: one end is passed to the
captain's `NewSessionRequest` as a stdio MCP server entry (command + args that
the agent will "spawn" — but it's actually the other end of the pipe pair held
by Ship). Ship reads MCP requests from the pipe, handles `ship_steer`,
`ship_accept`, and `ship_reject` calls, and writes MCP responses back. This
avoids needing a network listener — the captain connects to Ship's MCP server
the same way it connects to any other stdio MCP server.

### Captain Review Cycle

r[captain.review.auto]
In autonomous mode, when the captain calls `ship_steer`, the backend MUST
forward the steer directly to the mate without human approval. The human can
intervene at any time by sending their own steer or cancelling.

r[captain.review.human]
In human-in-the-loop mode, when the captain calls `ship_steer`, the backend
MUST present the steer to the human for approval before forwarding to the
mate. The human can approve as-is, edit the steer before sending, or discard
it and write their own.

### Human Direct Steer

r[captain.human-override]
When the human sends a steer directly (bypassing the captain), the backend
MUST inject the human's steer text into the captain's context for the next
review cycle. The captain MUST be informed that the human overrode its
direction — the injection MUST include a note like "The human sent this steer
directly to the mate, overriding your recommendation." The captain then
reviews the mate's output as normal after the mate finishes.

r[captain.human-override.cancel-pending]
If the captain had a pending steer (in `SteerPending` state) when the human
sends a direct steer, the captain's pending steer MUST be discarded. The
task transitions from `SteerPending` to `Working` as the mate processes the
human's steer.

## Mate Role

The mate is an AI agent whose job is implementation: writing code, running
tests, executing commands.

### Mate Prompting

r[mate.system-prompt]
The mate's system prompt MUST instruct it to act as an implementation-focused
engineer: write code, run tests, follow the captain's direction. It MUST
include the task description and any steer history.

r[mate.capabilities]
The mate agent MUST be configured with full ACP capabilities: filesystem
read/write, terminal create/output/kill/release.

r[mate.worktree-scope]
The mate MUST operate exclusively within the session's git worktree. Its
working directory is the worktree path.

### Mate Output

r[mate.output.streamed]
All mate output (message chunks, tool calls, plan updates) MUST be streamed
to the frontend in real time via the session event stream.

r[mate.output.persisted]
Mate output for each task MUST be persisted so it can be reviewed after the
fact and survives browser reloads.

r[captain.output.persisted]
Captain output (steer text, review comments, tool calls) MUST be persisted
alongside mate output. Both agents' content blocks are included in the event
replay (per `event.subscribe.replay`). If the browser reloads during a steer
review, the captain's proposed steer MUST survive and be re-presented to the
human.

## Approvals

The approval system gates agent actions that could be destructive or
irreversible.

### Permission Requests

r[approval.request.content]
When an agent sends a `RequestPermissionRequest`, the backend MUST extract the
tool name, arguments, and description and present them to the human via the UI.

r[approval.request.display]
Permission requests MUST be displayed inline in the agent's output stream,
at the point where the agent paused.

r[approval.request.actions]
The UI MUST offer at minimum: approve (once), deny, and approve-all-of-type
(for the remainder of this task).

r[approval.request.blocking]
The ACP `request_permission` call MUST block until the human responds. The
agent is paused during this time.

### Permission Policies

r[approval.policy.read-default]
File read operations SHOULD be auto-approved by default (configurable).

r[approval.policy.write-prompt]
File write operations MUST prompt the human unless auto-approved for the
session.

r[approval.policy.terminal-prompt]
Terminal command execution MUST prompt the human unless auto-approved for the
session.

r[approval.policy.session-memory]
Approval-all-of-type decisions MUST persist for the duration of the current
task only, resetting when a new task is assigned.

### Permission in Autonomous Mode

r[approval.autonomous]
Even in autonomous mode, permission requests from agents MUST still be
surfaced to the human. The captain auto-steers, but it does not auto-approve
destructive actions.

## Idle Reminders

When action is needed and nobody is acting, the system nudges.

r[idle.mate-done]
When the mate finishes (`StopReason::EndTurn`) and neither the captain nor
the human has acted within a configurable timeout (default: 2 minutes), the
system MUST emit an idle reminder event.

r[idle.permission-pending]
When a permission request has been pending for longer than a configurable
timeout (default: 1 minute), the system MUST emit an idle reminder event.

r[idle.ui-indicator]
Idle reminder events MUST be displayed prominently in the UI (visual pulse,
badge, or banner).

## Notifications

The system can notify the human outside the browser when attention is needed.

### Discord

r[notify.discord.webhook]
The system MUST support sending notifications to a Discord channel via
webhook URL, configurable per session or globally.

r[notify.discord.events]
Discord notifications MUST be sent for: permission requests pending, mate
task completion awaiting review, idle reminders, and agent errors.

r[notify.discord.content]
Discord notification messages MUST include the session name, event type, and
a brief summary (e.g., "Mate finished task: implement auth module — awaiting
review").

### Desktop

r[notify.desktop.support]
The UI MUST support browser desktop notifications (via the Notifications API)
when the browser tab is not focused.

r[notify.desktop.permission]
The UI MUST request notification permission from the browser on first use.

r[notify.desktop.events]
Desktop notifications MUST be sent for the same events as Discord
notifications.

### Sound

r[notify.sound.alert]
The UI MUST play an audio alert when a permission request arrives or when an
idle reminder fires, if the tab is not focused.

r[notify.sound.toggle]
Sound notifications MUST be togglable in the UI.

## Context Exhaustion

r[context.warning]
When an agent's context window drops below 20%, the UI MUST warn the human.

r[context.manual-rotation]
The human MUST be able to decide when to rotate an agent whose context is
exhausted.

## Autonomy Modes

r[autonomy.toggle]
Autonomy mode MUST be togglable per session.

### Human-in-the-Loop

r[autonomy.human-in-loop]
In human-in-the-loop mode (the default), the captain MUST propose steers and
the human MUST approve before they are sent to the mate.

### Autonomous

r[autonomy.autonomous]
In autonomous mode, the captain MUST auto-steer the mate. The human watches
the event stream and can intervene at any time.

r[autonomy.permission-gate]
The permission system MUST still gate destructive actions regardless of
autonomy mode.

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

## Projects

Ship manages multiple git repositories. Each repository is a "project."

r[project.registration]
Projects MUST be explicitly registered via the CLI (`ship project add`) or
the UI before sessions can be created in them. Ship does not auto-discover
repositories.

r[project.identity]
Each project MUST have a unique name derived from the repository directory
name (e.g., `/home/user/bearcove/roam` → `roam`). If a name collision occurs
during registration, Ship MUST append a disambiguating suffix.

r[project.validation]
On server startup, Ship MUST validate that all registered project paths still
exist and are git repositories. Projects with invalid paths MUST be flagged
in the UI (not silently removed).

r[project.persistence-dir]
Each project's `.ship/` directory (for session persistence, MCP config) is
relative to that project's repository root. Ship's own configuration (project
list, global settings) lives in `~/.config/ship/`.

r[project.mcp-defaults]
Each project MAY have a `.ship/mcp-servers.json` in its repository root for
project-specific MCP server defaults. A global default MAY also be configured
in `~/.config/ship/mcp-servers.json`. Project-level config takes precedence.

## Server Configuration

r[server.listen]
The server's HTTP listen address MUST be configurable via the `SHIP_LISTEN`
environment variable, defaulting to `[::]:9140`.

r[server.multi-repo]
A single Ship server instance manages sessions across all registered
projects. There is no need to run multiple Ship instances.

r[server.mode]
For v1, the server always runs in dev mode (Vite proxy enabled, hot reload).
Production mode is deferred to post-v1 (see `dev-proxy.prod-static`).

r[server.config-dir]
Ship's global configuration MUST be stored in `~/.config/ship/`. This
directory contains `projects.json` (the project registry) and optional
global settings.

r[server.discord-webhook]
The Discord webhook URL MUST be configurable via the `SHIP_DISCORD_WEBHOOK`
environment variable. If unset, Discord notifications are disabled.

r[server.agent-discovery]
On startup, Ship MUST check whether `claude-agent-acp` and `codex-acp`
binaries are available on `PATH`. The availability of each agent kind MUST
be surfaced in the create-session dialog — unavailable agent kinds MUST be
disabled with a `Tooltip` explaining what's missing (e.g., "codex-acp not
found on PATH").

## UI Design

This section specifies how each part of the UI is rendered, which Radix Themes
components are used, and how content blocks and interactions are laid out.

### Layout

r[ui.layout.shell]
The app shell MUST use a full-viewport `Flex` with `direction="column"`. A top
bar contains the app name and global controls. Below it, the main content area
fills the remaining space.

r[ui.layout.session-view]
The session view MUST use a `Grid` with two equal columns for the captain and
mate panels. On viewports narrower than 1024px, the grid MUST collapse to a
single column with a `Tabs` switcher (Captain | Mate) above.

```
┌──────────────────────────────────────────────────┐
│  Ship ∙ session-name          [mode] [close]     │  ← top bar
├────────────────────────┬─────────────────────────┤
│  Captain               │  Mate                   │
│  ┌──────────────────┐  │  ┌───────────────────┐  │
│  │ state + context  │  │  │ state + context   │  │  ← agent header
│  ├──────────────────┤  │  ├───────────────────┤  │
│  │                  │  │  │                   │  │
│  │  event stream    │  │  │  event stream     │  │  ← scrollable
│  │                  │  │  │                   │  │
│  │                  │  │  │                   │  │
│  └──────────────────┘  │  └───────────────────┘  │
├────────────────────────┴─────────────────────────┤
│  Task: implement auth module     [steer] [accept]│  ← task bar
└──────────────────────────────────────────────────┘
```

### Session List

r[ui.session-list.layout]
The session list MUST display sessions as a vertical stack of `Card` components,
each showing: project name (as `Badge`), session branch name, captain and mate
agent kinds (as `Badge`), current task description (truncated), task status
(as `Badge` with color), and a relative timestamp of last activity.

r[ui.session-list.project-filter]
The session list MUST include a project filter above the session cards. The
filter shows "All projects" by default and can be set to a specific project
via a `Select` dropdown. The filter state MUST be preserved in the URL query
string (e.g., `?project=roam`).

r[ui.session-list.empty]
When no sessions exist, the list MUST show a centered `Callout` with
instructions and a "New Session" `Button`. If no projects are registered,
the callout MUST instead prompt the user to add a project first, with an
"Add Project" `Button`.

r[ui.session-list.create]
The "New Session" action MUST open a `Dialog` containing a form with: project
(`Select` populated from registered projects), captain agent kind
(`SegmentedControl`: Claude | Codex), mate agent kind (`SegmentedControl`),
base branch (`Select` populated from the selected project's git branches),
and initial task description (`TextArea`). If only one project is registered,
it MUST be pre-selected.

r[ui.add-project.dialog]
The "Add Project" action (from the empty state callout or a top-bar button)
MUST open a `Dialog` containing: a `TextField` for the repository path
(absolute path, no tilde expansion — the backend resolves it), and a "Add"
`Button`. On submit, the dialog calls `proto.add-project`. If validation
fails (path doesn't exist, not a git repo, name collision), the error MUST
be displayed inline in the dialog as a red `Callout` without closing the
dialog.

r[ui.session-list.create.branch-filter]
The base branch `Select` MUST support type-to-filter for repositories with
many branches. If Radix `Select` proves insufficient, a `TextField` with
a filtered dropdown (combobox pattern) MUST be used instead.

r[ui.session-list.nav]
Each session `Card` MUST be a clickable link (using react-router `Link`)
that navigates to `/sessions/{session_id}`. The entire card surface MUST be
the click target, not a separate "View" button.

r[ui.session-list.status-colors]
Task status badges MUST use these Radix color scales: `Assigned` → gray,
`Working` → blue, `ReviewPending` → amber, `SteerPending` → orange,
`Accepted` → green, `Cancelled` → red.

### Agent Header

r[ui.agent-header.layout]
Each agent panel MUST start with a header row containing: a `Badge` showing
the agent kind (Claude or Codex), a state indicator, and a context usage bar.

r[ui.agent-header.state-indicator]
The agent state MUST be shown as a `Badge` with color coding: `Working` → blue
with a `Spinner` inline, `Idle` → gray, `AwaitingPermission` → amber,
`ContextExhausted` → red, `Error` → red with error icon.

r[ui.agent-header.context-bar]
Context remaining MUST be rendered as a `Progress` component. When below 20%,
the bar MUST switch to the red color scale and a `Callout` with variant "warning"
MUST appear below the header.

### Event Stream

r[ui.event-stream.layout]
Each agent panel MUST contain a `ScrollArea` displaying content blocks in
chronological order. The scroll area MUST auto-scroll to the bottom when new
content arrives, unless the user has scrolled up (sticky-scroll behavior).

r[ui.event-stream.grouping]
Adjacent text blocks from the same prompt turn are already merged into a single
block by the backend (per `event.block-id.text`). Tool calls are single blocks
with a lifecycle status (per `event.content-block.types`) — no client-side
grouping is needed. The frontend renders each block as-is from the store.

### Content Block: Text

r[ui.block.text]
Text content blocks MUST be rendered as markdown using a markdown renderer.
Inline code MUST use the Radix `Code` component. Block-level code fences MUST
be rendered with syntax highlighting (via a lightweight highlighter like
`shiki`). The surrounding container is a plain `Box` with body text styling.

### Content Block: Tool Call

r[ui.block.tool-call.layout]
A tool call block MUST be rendered as a single collapsible unit that updates
in place as patches arrive. While status is `pending` or `running`, the badge
shows a spinner. On `success` or `failure`, the badge updates to a checkmark
or X. The collapsed state shows one line: an icon, the tool name in a `Code`
span, and a status `Badge`. Clicking expands to show arguments and result.

```
▸ Read  src/auth.rs                              ✓
▾ Edit  src/auth.rs                              ✓
  ┌─────────────────────────────────────────────┐
  │ --- a/src/auth.rs                           │
  │ +++ b/src/auth.rs                           │
  │ @@ -10,3 +10,5 @@                           │
  │   fn validate() {                           │
  │ +     check_token();                        │
  │   }                                         │
  │                                             │
  └─────────────────────────────────────────────┘
```

r[ui.block.tool-call.collapsed-default]
Tool calls MUST be collapsed by default. File read tool calls MUST show the
file path in the collapsed line. File write/edit tool calls MUST show the file
path and a diff summary (e.g., "+3 -1").

r[ui.block.tool-call.diff]
File write and edit tool call results MUST render as unified diffs with
additions highlighted in green and deletions in red. The diff MUST use a
monospace `Code` block.

r[ui.block.tool-call.terminal]
Terminal tool calls (command execution) MUST show the command in the collapsed
line. The expanded view MUST show stdout/stderr in a monospace `Code` block
with a maximum height of 20rem and its own `ScrollArea`. Non-zero exit codes
MUST be shown as a red `Badge`.

r[ui.block.tool-call.search]
Search/grep tool calls MUST show the query in the collapsed line and match
results as a list of file:line snippets in the expanded view.

### Content Block: Plan Update

r[ui.block.plan.layout]
Plan updates MUST be rendered as an ordered list within a `Card`. Each step
shows its description and a status icon: planned (circle outline), in-progress
(`Spinner`), completed (check icon, green), failed (X icon, red).

r[ui.block.plan.position]
Plan updates MUST replace the previous plan in the stream, not append. Only
the latest plan is visible. It MUST be rendered as a sticky element at the
top of the agent panel's scroll area (below the header), not inline with
other content blocks.

r[ui.block.plan.filtering]
The frontend MUST filter `PlanUpdate` content blocks out of the chronological
event stream. They arrive as `ContentBlock` events (per
`event.content-block.types`) but render in the sticky plan area, not in the
scroll feed. All other content block types render in the chronological stream.

### Content Block: Error

r[ui.block.error]
Error content blocks MUST be rendered as a `Callout` with a red color scale,
an error icon, and the error message as body text. If the agent is in the
`Error` state, a "Retry" `Button` MUST appear inside the callout.

### Permission Request

r[ui.permission.layout]
Permission requests MUST be rendered inline in the event stream at the point
where the agent paused. They MUST use a `Card` with an amber border/background,
containing: the tool name in a `Code` span, a human-readable description of
the action, and the arguments in a collapsible detail.

r[ui.permission.actions]
The permission card MUST contain three `Button` components: "Approve" (solid,
green), "Deny" (soft, red), and "Approve all [tool name]" (outline, green).
The "Approve all" button includes a `Tooltip` explaining it applies for the
remainder of the current task.

r[ui.permission.resolved]
After resolution, the permission card MUST update in-place: approved requests
show a green check `Badge`, denied requests show a red X `Badge`. The action
buttons MUST be removed.

r[ui.permission.viewer-mode]
In viewer mode (non-controlling browser), permission action buttons MUST be
disabled with a `Tooltip` explaining "Another browser controls this session."

### Steer Review (Human-in-the-Loop)

r[ui.steer-review.layout]
When the captain produces a steer in human-in-the-loop mode, the steer MUST
appear as a `Card` at the bottom of the session view (above the task bar),
containing: the captain's steer text rendered as markdown, and three action
buttons.

r[ui.steer-review.actions]
The steer review card MUST contain: "Send to Mate" `Button` (solid, blue)
which forwards the steer as-is, "Edit & Send" `Button` (outline, blue) which
opens the steer text in an editable `TextArea` before sending, and "Discard"
`Button` (soft, red) which discards the captain's steer entirely.

r[ui.steer-review.edit-mode]
When "Edit & Send" is clicked, the steer text MUST be replaced by a `TextArea`
pre-filled with the captain's text. A "Send" `Button` and "Cancel" `Button`
appear below. The human can modify the text freely before sending.

r[ui.steer-review.own-steer]
A "Write your own steer" `Button` (outline, gray) MUST always be available in
the task bar, allowing the human to bypass the captain and steer the mate
directly. This opens a `Dialog` with a `TextArea` for the steer message.

### Task Bar

r[ui.task-bar.layout]
The task bar MUST be a horizontal `Flex` pinned to the bottom of the session
view, containing: the current task description (truncated with `Tooltip` for
full text), the task status as a colored `Badge`, and action buttons.

r[ui.task-bar.actions]
Task bar actions depend on task status:
- `Working` → "Cancel" `Button` (soft, red)
- `ReviewPending` → "Accept" `Button` (solid, green), "Cancel" `Button` (soft,
  red), plus the steer review card above if in human-in-the-loop mode
- `SteerPending` → steer review card is the primary action
- `Idle` (no active task) → "New Task" `Button` (solid, blue) which opens a
  `Dialog` with a `TextArea`

r[ui.task-bar.new-task]
The "New Task" button in idle state MUST call `proto.assign` with the task
description from the dialog. This is the same operation used at session
creation — there is no separate "subsequent task" operation.

r[ui.task-bar.history]
A "History" `IconButton` MUST open a `Popover` showing the session's completed
tasks as a `DataList` with task descriptions, statuses, and timestamps.

### Idle Reminders

r[ui.idle.banner]
Idle reminder events MUST be rendered as a pulsing `Callout` with an amber
color scale, appearing at the top of the session view. The callout MUST
describe what is waiting (e.g., "Mate finished — awaiting review" or
"Permission request pending for 2 minutes").

r[ui.idle.badge]
In the session list, sessions with pending idle reminders MUST show a pulsing
amber dot next to the session card.

### Notifications

r[ui.notify.desktop-prompt]
On first visit, the UI MUST display a `Callout` asking the user to enable
desktop notifications, with an "Enable" `Button` that calls
`Notification.requestPermission()`.

r[ui.notify.sound-toggle]
The top bar MUST contain an `IconButton` (speaker icon) that toggles sound
notifications on/off. The current state MUST be persisted in `localStorage`.

### Error States

r[ui.error.agent]
When an agent is in the `Error` state, its entire panel MUST show a `Callout`
with the error message and a "Retry" `Button`. The event stream remains
visible but grayed out below the error callout.

r[ui.error.connection]
If the WebSocket connection drops, a full-width `Callout` with red color scale
MUST appear at the top of the page: "Connection lost — reconnecting..." with
a `Spinner`. On reconnection, it MUST disappear automatically.

### Autonomy Mode Toggle

r[ui.autonomy.toggle]
The session view top bar MUST contain a `Switch` labeled "Autonomous" that
toggles the session's autonomy mode. The current mode MUST also be shown as
a `Badge` (gray for human-in-the-loop, blue for autonomous).

### Theme Configuration

r[ui.theme.config]
The Radix `Theme` provider MUST be configured with: `appearance="dark"`,
`accentColor="iris"`, `grayColor="slate"`, `radius="medium"`,
`scaling="100%"`.

r[ui.theme.dark-only]
Ship is dark mode only. There MUST NOT be a theme switcher or light mode
support. The `appearance` prop is hardcoded to `"dark"`.

r[ui.theme.font]
The app MUST use a monospace font stack for all code-related content (diffs,
terminal output, tool arguments) and the system sans-serif stack for UI text.
Font configuration MUST be applied via vanilla-extract global styles, not
Radix theme overrides.

### Keyboard Shortcuts

r[ui.keys.permission]
When a permission request is focused or the most recent pending request,
pressing `Enter` MUST approve it and `Escape` MUST deny it.

r[ui.keys.steer-send]
In the steer review card and the "write your own steer" dialog, `Cmd+Enter`
(macOS) / `Ctrl+Enter` (other platforms) MUST submit the steer.

r[ui.keys.cancel]
`Escape` MUST close any open `Dialog` or `Popover` (this is Radix default
behavior, listed here for completeness).

r[ui.keys.nav]
`1` and `2` MUST switch focus between the captain and mate panels when no
text input is focused.

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

## Cost Tracking

r[cost.not-tracked]
Ship intentionally does NOT track token usage or API costs. Both Claude and
Codex are expected to be used via subscriptions (Claude Pro/Team, Codex
subscription), not metered API tokens. If a future agent kind requires API
billing, cost tracking can be added then.
