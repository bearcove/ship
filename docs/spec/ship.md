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
The system MUST allow creating a session with a specified captain agent kind,
mate agent kind, and base branch.

r[session.list]
The system MUST allow listing all active sessions with their current state.

r[session.persistent]
Sessions MUST be persistent across browser reloads and server restarts.

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
`TaskId`), enums (`AgentKind`, `Role`, `AgentState`), event types
(`SessionEvent`), and request/response structs. This crate MUST NOT depend on
any runtime or framework crates.

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

r[backend.task-persistence]
Task state MUST be persisted to survive server restarts.

r[backend.persistence-format]
Session and task state MUST be serialized using facet-json and stored as JSON
files in a `.ship/` directory relative to the repository root. Each session
gets a `{session_id}.json` file containing the session config, current task,
and task history.

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
In production mode (no dev proxy), the backend MUST serve the frontend from
Vite's build output directory, with SPA fallback to `index.html`.

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
Session identifiers MUST be UUIDs wrapped in a `SessionId` newtype.

r[proto.id.task]
Task identifiers MUST be UUIDs wrapped in a `TaskId` newtype.

### Operations

r[proto.create-session]
The protocol MUST support a `create_session` operation that creates a new
session with a given agent configuration and returns a `SessionId`.

r[proto.list-sessions]
The protocol MUST support a `list_sessions` operation that returns summaries
of all active sessions.

r[proto.assign]
The protocol MUST support an `assign` operation where the captain assigns a
task to the mate, returning a `TaskId`.

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

r[proto.close-session]
The protocol MUST support a `close_session` operation that tears down both
agents, triggers worktree cleanup (with confirmation), and removes the session
from the active list. The session's persistence file is retained for history.

r[proto.get-session]
The protocol MUST support a `get_session` operation that returns the full
session state: agent snapshots, current task (with all content blocks and
steer history), task history, and autonomy mode. This is used for initial
hydration when a browser connects or reconnects.

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
characters of the session UUID and `slug` is a kebab-case summary derived from
the session's first task description (or "untitled" if no task yet).

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
browsers see the full current state without a separate hydration call.

### Event Types

r[event.agent-state-changed]
The system MUST emit `AgentStateChanged` events when an agent's state changes,
including the role and new state.

r[event.content-block]
The system MUST emit `ContentBlock` events when an agent produces content
(text, tool call, diff, etc.), including the role and block data.

r[event.permission-requested]
The system MUST emit `PermissionRequested` events when an agent requests
permission for an action.

r[event.task-status-changed]
The system MUST emit `TaskStatusChanged` events when a task's status changes,
including the task ID and new status.

r[event.context-updated]
The system MUST emit `ContextUpdated` events when an agent's context usage
changes, including the role and remaining percentage.

## Resilience

r[resilience.reconnect]
If the ACP connection drops mid-task, the backend MUST reconnect and call
`LoadSession` to resume.

r[resilience.state-in-backend]
Task state MUST live in the backend, not in the agent, so nothing is lost on
disconnection.

r[resilience.context-recovery]
If the agent lost context (process crash), the backend MUST inject a summary
prompt to catch it up.

## Session Sharing

r[sharing.multi-browser]
Multiple browsers MUST be able to watch the same session simultaneously.

r[sharing.event-broadcast]
Every connected client MUST receive the same `SessionEvent`s via roam's
multi-subscriber support.

r[sharing.single-writer]
Steering MUST be single-writer: one active controller per session at a time.

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

r[captain.steer-output]
The captain's prompt response MUST be interpreted as a steer instruction for
the mate. The backend extracts the captain's text output and sends it as the
next `steer` to the mate.

r[captain.no-tools]
The captain agent MUST be configured without filesystem or terminal
capabilities. It reviews output, it does not execute.

### Captain Review Cycle

r[captain.review.auto]
In autonomous mode, when the captain produces a steer, the backend MUST
forward it directly to the mate without human approval. The human can
intervene at any time by sending their own steer or cancelling.

r[captain.review.human]
In human-in-the-loop mode, when the captain produces a steer, the backend
MUST present it to the human for approval before forwarding to the mate.
The human can approve as-is, edit the steer before sending, or discard it
and write their own.

r[captain.accept-signal]
The captain MUST be able to signal that the task is complete by including a
structured accept marker in its response (e.g., a specific JSON block or
keyword). The backend MUST detect this and transition the task to accepted.

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

## Cost Tracking

r[cost.not-tracked]
Ship intentionally does NOT track token usage or API costs. Both Claude and
Codex are expected to be used via subscriptions (Claude Pro/Team, Codex
subscription), not metered API tokens. If a future agent kind requires API
billing, cost tracking can be added then.
