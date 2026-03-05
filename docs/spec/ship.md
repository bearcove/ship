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

## Architecture

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

### Frontend

r[frontend.typescript]
The frontend MUST be implemented in TypeScript.

r[frontend.react]
The frontend MUST use React for UI rendering.

r[frontend.vite]
The frontend MUST use Vite as its dev server and build tool.

r[frontend.codegen]
Frontend types MUST be generated from the backend's Rust traits via
roam-codegen.

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

r[worktree.base-branch]
Worktrees MUST be created from a user-specified base branch when the session
starts.

r[worktree.shared]
Both agents in a session MUST operate within the same worktree.

r[worktree.cleanup]
Worktrees MUST be cleaned up on session close, with confirmation from the
human.

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
