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
agent kind, mate agent kind, and base branch. `create_session` MUST persist
the session record and return promptly. Worktree creation, agent startup,
captain bootstrap, and related initialization continue in the background and
are surfaced through the session state and event stream. A newly created
session starts with no active task.

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
The UI displays task status (description and current state) inline in the agent
panels. There is no separate task panel or explicit task-creation control; the
human interacts with the captain directly via the inline composer.

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
name, agent configuration, and base branch. It creates durable session state
and returns a `SessionId` promptly. Worktree creation, agent startup, and
captain bootstrap continue asynchronously in the background.

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

r[proto.steer]
The protocol MUST support a `steer` operation where the human explicitly sends
or approves new direction for the mate. This is the backend entrypoint that
starts a mate turn. Captain-originated delegation via `ship_steer` uses the
same underlying mate-delegation path once the steer is approved.

r[proto.accept]
The protocol MUST support an `accept` operation where the captain accepts the
mate's work and closes the task.

r[proto.cancel]
The protocol MUST support a `cancel` operation that cancels the current task.

r[proto.interrupt-captain]
The protocol MUST support an `interrupt_captain` operation that cancels any
in-flight captain response without affecting task state. This is used by the
UI when the user presses Escape while the captain is working.

r[proto.resolve-permission]
The protocol MUST support a `resolve_permission` operation to respond to agent
permission requests.

r[proto.reply-to-human]
The protocol MUST support a `reply_to_human` operation that takes a session ID
and a message string. This unblocks a `captain_notify_human` call that is
waiting for human input.

r[proto.set-agent-effort]
The protocol MUST support a `set_agent_effort` operation that takes a session
ID, role, config ID, and value ID. This changes the thinking-effort level of
the specified agent via the ACP `set_session_config_option` call and emits an
`AgentEffortChanged` event so all subscribers see the update.

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
Frontend hydration MUST follow this sequence:
1. Call `get_session` for structural state (project, branch, agent kinds,
   autonomy mode, task metadata, agent snapshots). This populates the UI
   shell immediately — the user sees the layout before content loads.
2. Open the event subscription channel via `subscribe_events`. The backend
   replays the current task's event log (per `event.subscribe.replay`),
   which the client-side reducer (per `event.client.reducer`) processes to
   build the block stores and derive agent/task states.
3. After replay, the backend sends `ReplayComplete`. The frontend
   transitions from "loading" to "live" and begins processing live events.

The two mechanisms are complementary: `get_session` provides the skeleton
(cheap, fast), the event channel provides the content (progressive). The
frontend MUST NOT attempt to render content blocks from `get_session` — all
block data comes exclusively from the event stream.

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
Each plan step MUST have a description, a priority (high, medium, low), and
a status: `Pending`, `InProgress`, `Completed`, or `Failed`. These map from
ACP's `PlanEntryStatus` (`Pending`, `InProgress`, `Completed`) with `Failed`
added by Ship for steps that errored.

### Snapshot

r[agent-state.snapshot]
An `AgentSnapshot` MUST include the agent's role, kind, state, and an optional
context remaining percentage (0-100).

## Task Lifecycle

### Task Status

r[task.status.enum]
A task's status MUST be one of the following values:

- `Assigned` — task has been created and is waiting for captain direction
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
Task creation begins when the captain calls the `captain_assign` MCP tool with
a task description. There is no human-facing assign RPC; the captain is the
sole entry point for creating tasks.

r[task.prompt]
On assignment, the backend MUST send a `SessionPrompt` to the captain via ACP.

r[task.progress]
While the mate works, the backend MUST receive ACP notifications and stream
progress to the frontend in real time.

r[task.completion]
When the mate calls `mate_submit(summary)`, the task moves to `ReviewPending`
and the backend MUST automatically prompt the captain with the mate's summary
for review. The captain then calls `captain_accept`, `captain_steer`, or
`captain_cancel`.

r[task.completion.enforce-submit]
If the mate stops (`StopReason::EndTurn`) without having called `mate_submit`,
the backend MUST re-prompt the mate with instructions to call `mate_submit`
with a summary of completed work. The mate MUST NOT be allowed to finish
without submitting.

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
Event subscription MUST be implemented as a roam `Tx<SubscribeMessage>`
channel on the `subscribe_events` method of the `Ship` service trait. The
frontend opens the channel by session ID and receives a stream of
`SubscribeMessage` values. `SubscribeMessage` is a wire-level enum defined
in `ship-types` with the following variants:

- `Event(SessionEvent)` — a sequenced session event (replayed or live)
- `ReplayComplete` — subscription-local control marker (no seq field)

The `Snapshot` variant is reserved for the post-v1 optimization described
in `event.replay.snapshot-optimization` and MUST NOT be implemented in v1.

This explicit union type ensures the backend and frontend agree on what the
channel carries, with no ambiguity about out-of-band vs in-band delivery.

r[event.subscribe.replay]
When a new subscriber connects, the backend MUST replay all `SessionEvent`s
from the current task's event log before streaming live events. This includes
block events (`BlockAppend`, `BlockPatch`) AND top-level events
(`AgentStateChanged`, `TaskStatusChanged`, `ContextUpdated`, `TaskStarted`).
The replay starts from the `TaskStarted` event for the current task — NOT
from `seq 0`. This ensures late-joining browsers see the full current-task
state without a separate hydration call. Replay events use the same types and
sequence numbers as originally emitted — the frontend does not distinguish
between replayed and live events.

### Event Envelope

r[event.envelope]
Every `SessionEvent` sent to the frontend MUST be wrapped in an envelope
containing: a monotonically increasing sequence number (`seq`), and the event
payload. The sequence number is per-session, starts at 0 when the session is
created, and never resets — it increases across task boundaries. Replay events
use the original sequence numbers. Events that are scoped to a role include
the role in their payload; the envelope itself is role-agnostic.

Only `SubscribeMessage::Event(SessionEvent)` carries a sequence number.
`SubscribeMessage::ReplayComplete` has no seq field. Only `SessionEvent`s
are persisted in the event log and broadcast to all subscribers.

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
For tool calls, the backend MUST generate a fresh `BlockId` (ULID) on the
first `ToolCall` notification and maintain a `ToolCallId -> BlockId` lookup
map for the duration of the task. Subsequent `ToolCallUpdate` notifications
are matched to their block via this map.

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
Permission request blocks MUST get a fresh `BlockId` (ULID). The block MUST
also store the `ToolCallId` from the ACP `RequestPermissionRequest` so the
frontend can visually associate it with the related tool call block.

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

r[event.task-started]
The system MUST emit a `TaskStarted` event when a new task is assigned. The
payload includes the task ID and task description. On receiving this, the
frontend MUST clear both block stores. The sequence number does NOT reset —
it continues from the session's current value.

r[event.agent-effort-changed]
The system MUST emit an `AgentEffortChanged` event when an agent's thinking
effort configuration changes — either at spawn time (reflecting the ACP
session's initial `ThoughtLevel` config) or in response to `set_agent_effort`.
The payload includes the role, config ID, current value ID, and the full list
of available effort values with their display names.

r[event.human-review-requested]
The system MUST emit a `HumanReviewRequested` event when the captain calls
`captain_notify_human`. The payload includes the captain's message, a unified
diff of changes since the base branch, and the worktree path. The frontend
displays this as a blocking review panel.

r[event.human-review-cleared]
The system MUST emit a `HumanReviewCleared` event when the human replies via
`reply_to_human`, clearing the pending review state on all subscribers.

r[event.replay-complete]
After the backend has sent all replayed events to a newly connected
subscriber, it MUST send `SubscribeMessage::ReplayComplete` on that
subscriber's channel (per `event.subscribe.roam-channel`).
`ReplayComplete` is a subscription-local signal — it is NOT persisted in
the task event log, NOT broadcast to other subscribers, and does NOT carry
a sequence number. It is a variant of `SubscribeMessage`, not a
`SessionEvent`. The frontend uses it to transition from "loading history"
to "live".

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
  where each step has `description`, `status` (`Pending`/`InProgress`/`Completed`/`Failed`),
  and `priority` (`High`/`Medium`/`Low`).
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

r[event.store.immutable-updates]
Block store updates MUST use immutable data patterns (new object references
on mutation) so that React can detect changes via reference equality. Do NOT
mutate block objects in place.

### Replay Semantics

r[event.replay.same-events]
For v1, replay MUST send all `SessionEvent`s from the current task's event
log — block events and top-level events — in the same order, with the same
sequence numbers as originally emitted. The first replayed event is the
`TaskStarted` for the current task; the last is the most recent event before
the subscriber connected. The frontend processes them with the same reducer
as live events. There is no special "replay mode."

r[event.replay.followed-by-marker]
After all replayed `SessionEvent`s have been sent to a subscriber, the
backend MUST send a `ReplayComplete` control marker (per
`event.replay-complete`) on that subscriber's channel. The frontend SHOULD
show a loading indicator until `ReplayComplete` is received. Since
`ReplayComplete` is not a `SessionEvent`, it does not appear in the event
log and is not broadcast to other subscribers.

r[event.replay-batch]
The frontend MUST buffer all events received during replay and apply them
in a single batch dispatch when `ReplayComplete` arrives. This avoids
per-event React render cycles during replay, reducing session switch cost
from O(N) renders to O(1). The batch reducer MUST use mutable block store
operations internally to avoid O(N²) array copies, then produce a single
immutable state at the end.

r[event.replay.snapshot-optimization]
As a post-v1 optimization, the backend MAY replace event-by-event replay
with a single `Snapshot` control message containing the full materialized
session state: both block stores, agent states, task status, context levels,
and the current sequence number. This gives the subscriber enough to
initialize its `SessionViewState` without processing individual events. If
implemented, `Snapshot` MUST be followed by `ReplayComplete`, and the
subscriber MUST set `lastSeq` from the snapshot so it can detect gaps in
subsequent live events. Like `ReplayComplete`, `Snapshot` is a
subscription-local control message, not a `SessionEvent`. This is NOT
required for v1 and MUST NOT be implemented until the event-by-event replay
is proven to be a bottleneck.

### Multi-Subscriber Replay Behavior

r[event.replay.per-subscriber]
Replay is per-subscriber. When a new subscriber connects (whether a
reconnecting browser or a late-joining second browser), the backend replays
the event log for that subscriber independently. Each subscriber receives
its own `ReplayComplete` marker after its own replay finishes. Other
already-connected subscribers are unaffected — they continue receiving live
events without interruption.

Example: reconnect flow for a single browser (session is on task 2,
`TaskStarted` for task 2 was at `seq 30`).

1. Browser connects, subscribes to session `S`.
2. Backend replays events `seq 30..47` (current task's log).
3. Backend sends `ReplayComplete` (no seq) on this subscriber's channel.
4. Browser processes events, sets `replayComplete = true`, renders.
5. Live events `seq 48, 49, ...` arrive as they are produced.
6. WebSocket drops. Browser sets `connected = false`, clears state.
7. Browser reconnects, re-subscribes to session `S`.
8. Backend replays events `seq 30..52` (log has grown).
9. Backend sends `ReplayComplete`.
10. Browser is caught up again.

Example: late-joining second browser (same session, same task 2).

1. Browser A is connected and live at `seq 50`.
2. Browser B connects, subscribes to session `S`.
3. Backend replays events `seq 30..50` to Browser B only.
4. Backend sends `ReplayComplete` to Browser B only.
5. A new live event `seq 51` is produced.
6. Both Browser A and Browser B receive `seq 51` — it is a normal
   `SessionEvent` broadcast to all subscribers.

### Replay and Broadcast Invariants

r[event.replay.invariants]
Implementers MUST verify the following invariants:

- The task event log contains only `SessionEvent`s. `ReplayComplete` and
  `Snapshot` MUST NOT appear in the log.
- Every `SessionEvent` produced by the event pipeline (per
  `backend.event-pipeline`) is appended to the log AND broadcast to all
  current subscribers. No event is logged without broadcast or vice versa.
- `ReplayComplete` is sent exactly once per subscriber per subscription
  (including re-subscriptions after reconnect).
- Two subscribers connected to the same session at the same time receive
  identical live `SessionEvent`s with identical sequence numbers.
- A subscriber that connects, receives replay + `ReplayComplete`, and then
  receives live events MUST observe a contiguous, gap-free sequence from
  the `TaskStarted` event of the current task through the latest live event.
  The first replayed seq is NOT necessarily 0 — it is the seq of the current
  task's `TaskStarted` event.

r[event.replay.test-mixed-stream]
The backend and frontend MUST each have integration tests that exercise a
mixed `SubscribeMessage` stream: a sequence of `Event(...)` items followed
by `ReplayComplete`, then further `Event(...)` items. Tests MUST assert
correct decoding/handling of both variants, and that `ReplayComplete` is
processed exactly once per subscription without affecting the event log or
other subscribers.

### Client-Side Session State

The frontend does not just store a list of events. It maintains a
`SessionViewState` — a structured, consistent view of the session that is
derived from the event stream. Every event is processed by a pure reducer
function: `(state, event) -> state`.

r[event.client.view-state]
The frontend MUST maintain a `SessionViewState` for each open session
containing:
- `captainBlocks`: ordered block store for the captain role
- `mateBlocks`: ordered block store for the mate role
- `captainState`: current `AgentState` for the captain
- `mateState`: current `AgentState` for the mate
- `taskStatus`: current task status (or null if no active task)
- `taskId`: current task ID (or null)
- `captainContext`: context remaining percentage (or null)
- `mateContext`: context remaining percentage (or null)
- `lastSeq`: last processed sequence number
- `replayComplete`: boolean, false until `ReplayComplete` is received
- `connected`: boolean, WebSocket connection status

r[event.client.reducer]
The `SessionViewState` MUST be updated by a pure reducer function that
handles every `SessionEvent` variant. The reducer MUST be a pure function
with no side effects — given the same state and event, it always produces
the same new state. This makes the system testable and predictable.

The reducer MUST handle every `SessionEvent` variant:
- `BlockAppend` → insert block into the appropriate role's store
- `BlockPatch` → find block by ID in the appropriate role's store, apply
  the patch, produce a new block object (immutable update)
- `AgentStateChanged` → update `captainState` or `mateState`
- `TaskStatusChanged` → update `taskStatus`
- `ContextUpdated` → update `captainContext` or `mateContext`
- `TaskStarted` (per `event.task-started`) → clear both block stores, set
  new `taskId` and `taskStatus`. `lastSeq` is NOT reset.

`ReplayComplete` is NOT a `SessionEvent` and MUST NOT be processed by the
reducer. The subscription layer sets `replayComplete = true` when it
receives the control marker, outside the reducer.

r[event.client.reducer-purity]
The reducer MUST NOT call APIs, dispatch actions, or trigger side effects.
Side effects (e.g., playing a notification sound, showing a desktop
notification) MUST be handled by a separate listener that observes state
transitions, not by the reducer itself.

r[event.client.connection-lifecycle]
When the WebSocket connection drops, the frontend MUST set `connected` to
false and show a connection error banner (per `ui.error.connection`). When
the connection is re-established, the frontend MUST clear the
`SessionViewState` and re-subscribe. The replay will rebuild the full state.
The frontend MUST NOT attempt to merge new events with stale state from a
previous connection.

r[event.client.hydration-sequence]
On navigating to a session view, the frontend MUST:
1. Call `get_session` to get the structural skeleton (project, branch,
   agent kinds, autonomy mode, task metadata). This populates the UI
   chrome immediately.
2. Open the event subscription channel. The backend replays events, which
   the reducer processes to build the block stores and update agent states.
3. On `ReplayComplete`, the UI transitions from "loading" to "live".

This two-phase approach (per `proto.hydration-flow`) means the user sees
the session layout immediately while content loads progressively.

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
Every connected subscriber MUST receive the same `SessionEvent`s (with the
same sequence numbers) for live events. Subscription-local control markers
(`ReplayComplete`) are per-subscriber and are NOT broadcast.

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
The captain's bootstrap prompt MUST instruct it to act as a senior engineer
who scopes work, reviews output, and delegates to the mate when appropriate.
It MUST NOT instruct the captain to write code directly.

r[captain.initial-prompt]
When a task is assigned, the backend MUST prompt the captain with the task
description. The captain decides whether to ask clarifying questions in text,
call `ship_steer`, call `ship_accept`, or call `ship_reject`.

r[captain.context]
The captain MUST be able to inspect the current task state and the mate's
streamed output through Ship's session view and event history. Ship MUST keep
the captain's conversation long-lived across prompts within the session.

r[captain.capabilities]
The captain agent MUST NOT be configured with raw ACP filesystem or terminal
capabilities. It reviews output, it does not execute. The captain does have
access to Ship's captain MCP tools (`captain_assign`, `captain_steer`,
`captain_accept`, `captain_cancel`, `captain_notify_human`) — these are MCP
tools, not ACP filesystem/terminal capabilities.

### Captain Tools

The captain communicates decisions to Ship via MCP tools on its session.
The human discusses goals with the captain; the captain assigns tasks to the
mate and manages the work cycle. This avoids fragile text parsing.

r[captain.tool.implementation]
Captain tools MUST be implemented as MCP tools served by Ship itself. The
captain's `NewSessionRequest` MUST include Ship's captain MCP server in its
`mcp_servers` list so the captain can discover and call these tools.

r[captain.tool.transport]
Ship MUST expose captain MCP tools via a per-session stdio proxy. For each
captain session, Ship spawns a dedicated stdio MCP server process and passes
it in the captain's `NewSessionRequest`. No public network listener is
required.

r[captain.tool.assign]
The captain MUST have access to a `captain_assign` tool that takes a
`description` argument (string) and an optional `keep` argument (boolean,
default false). When called, the backend creates a new task and starts the mate
working on it immediately. If `keep` is false or omitted, the mate's ACP
session is torn down and a new one is started (fresh context). If `keep` is
true, the existing mate ACP session is reused.

r[captain.tool.steer]
The captain MUST have access to a `captain_steer` tool that takes a `message`
argument (string). This is fire-and-forget: if the mate is blocked on
`mate_ask_captain` or `mate_submit`, the message resolves that pending call;
otherwise it is injected into the mate's stream directly.

r[captain.tool.accept]
The captain MUST have access to a `captain_accept` tool that takes an optional
`summary` argument (string). When called, the backend resolves any pending
`mate_submit` with an accepted outcome and transitions the task to accepted.

r[captain.tool.cancel]
The captain MUST have access to a `captain_cancel` tool that takes an optional
`reason` argument (string). The backend cancels the mate's in-progress work,
resolves any pending `mate_submit` with a cancelled outcome, and transitions
the task to `Cancelled`.

r[captain.tool.notify-human]
The captain MUST have access to a `captain_notify_human` tool that takes a
`message` argument (string). This blocks until the human responds via the UI,
then returns the human's reply text to the captain.

r[captain.tool.read-only]
The captain MUST have access to read-only file tools: `read_file`,
`search_files`, and `list_files`. These operate on the session worktree with
the same semantics as the mate equivalents (see r[mate.tool.read-file],
r[mate.tool.search-files], r[mate.tool.list-files]). The captain MUST NOT
have access to write, edit, or run-command tools.

### Mate Tools

r[mate.tool.implementation]
Mate tools MUST be implemented as MCP tools served by Ship itself. The mate's
`NewSessionRequest` MUST include Ship's mate MCP server in its `mcp_servers`
list. A separate per-session stdio proxy is spawned for the mate.

r[mate.tool.read-file]
The mate MUST have access to a `read_file` tool that takes a worktree-relative
`path` argument (string), plus optional 1-based `offset` and `limit`
arguments. Ship MUST reject absolute paths, directory traversal, directories,
binary files, and missing files with clear errors. For text files, Ship MUST
return numbered lines in a code-reading format, truncate individual lines
longer than 2000 characters, and truncate output to at most the requested line
window with a message explaining how to read more.

r[mate.tool.write-file]
The mate MUST have access to a `write_file` tool that takes `path` (relative
to worktree) and `content` arguments. For Rust files, the backend MUST write
the candidate file into place, run `rustfmt`, and restore the previous file
contents if formatting fails; if validation fails the file is not written and
the error is returned. Valid Rust files are auto-formatted. Parent directories
are created as needed.

r[mate.tool.edit-prepare]
The mate MUST have access to an `edit_prepare` tool that takes `path`,
`old_string`, and `new_string` arguments, plus an optional `replace_all`
flag. The tool computes the replacement and returns a unified diff preview
with an `edit_id`. The file is not modified. The `old_string` must match
exactly once unless `replace_all` is true.

r[mate.tool.edit-confirm]
The mate MUST have access to an `edit_confirm` tool that takes an `edit_id`.
The tool applies the prepared edit, runs syntax validation for Rust files,
and returns success or error. If validation fails or the file was modified
since prepare, the edit is rejected and the file is unchanged.

r[mate.tool.search-files]
The mate MUST have access to a `search_files` tool that takes raw ripgrep
arguments as a string. The command runs in the worktree root. Output is
truncated at 1000 lines.

r[mate.tool.list-files]
The mate MUST have access to a `list_files` tool that takes raw fd arguments
as a string. The command runs in the worktree root. Output is truncated at
1000 lines.

r[mate.tool.run-command]
The mate MUST have access to a `run_command` tool that takes a `command`
argument (string) and optional `cwd` (relative to worktree). The command
runs via `sh -c` in the worktree per `r[mate.tool.sandbox]`. Commands
matching dangerous patterns per `r[mate.tool.guardrails]` are not executed;
instead the mate is directed to explain the need to the captain via
`mate_ask_captain`. Output is truncated at 1000 lines. Timeout is 120
seconds.

r[mate.tool.sandbox]
On macOS, Ship MUST execute mate `run_command` calls under `sandbox-exec`
with a Seatbelt profile that:
- Allows all filesystem reads
- Denies all filesystem writes outside the session worktree and `/tmp`
- Denies all network access (including outbound TCP/UDP)
On other platforms, sandboxing is not yet implemented.

r[mate.tool.networked-sandbox]
Certain mate tools require network access (e.g. to fetch packages). These
tools run under a separate `sandbox-exec` profile identical to
`r[mate.tool.sandbox]` except network access is permitted. Tools covered:
`cargo_check`, `cargo_clippy`, `cargo_test`, `pnpm_install`.

r[mate.tool.cargo-check]
The mate MUST have access to a `cargo_check` tool that runs `cargo check`
in the session worktree under the networked sandbox (`r[mate.tool.networked-sandbox]`).
Takes an optional `args` string appended to the command.

r[mate.tool.cargo-clippy]
The mate MUST have access to a `cargo_clippy` tool that runs `cargo clippy`
in the session worktree under the networked sandbox. Takes an optional
`args` string appended to the command.

r[mate.tool.cargo-test]
The mate MUST have access to a `cargo_test` tool that runs `cargo nextest run`
in the session worktree under the networked sandbox. Takes an optional
`args` string appended to the command.

r[mate.tool.pnpm-install]
The mate MUST have access to a `pnpm_install` tool that runs `pnpm install`
in the session worktree under the networked sandbox. Takes an optional
`args` string appended to the command.

r[mate.tool.send-update]
The mate MUST have access to a `mate_send_update` tool that takes a `message`
argument (string). The message is injected into the captain's context as a
user message and the captain is prompted. Returns immediately without waiting
for a response.

r[mate.tool.plan-create]
The mate MUST have access to a `plan_create` tool that takes a `steps`
argument (`Vec<String>`). The mate MUST call this before starting substantive
work. The backend persists the plan, auto-commits any pending worktree
changes, and asynchronously notifies the captain with the full plan without
blocking the mate on captain review.

r[mate.tool.plan-step-complete]
The mate MUST have access to a `plan_step_complete` tool that takes a
`step_index` argument (`usize`) and a `summary` argument (string). The backend
marks the indexed plan step completed, auto-commits any pending worktree
changes with a commit message derived from the step description and summary,
and asynchronously notifies the captain with the updated plan plus commit
details.

r[mate.tool.ask-captain]
The mate MUST have access to a `mate_ask_captain` tool that takes a `question`
argument (string). The question is injected into the captain's context and the
captain is prompted. This call blocks until the captain calls `captain_steer`,
at which point the captain's message is returned as the answer.

r[mate.tool.submit]
The mate MUST have access to a `mate_submit` tool that takes a `summary`
argument (string). The mate calls this when it believes its work is complete.
The backend transitions the task to `ReviewPending`, notifies the captain, and
blocks until the captain responds:
- `captain_accept` → returns an accepted message; task transitions to accepted.
- `captain_steer` → returns captain feedback; mate continues working.
- `captain_cancel` → returns a cancellation error; task transitions to cancelled.

r[mate.tool.guardrails]
Ship MUST block dangerous mate commands that can destroy work, including git
reset/restore/checkout/clean and broad recursive deletion commands. When such
a command is blocked, Ship MUST reject the tool call and steer the mate to
stop current work and explain the situation to the captain.

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
directly to the mate, overriding your recommendation." Ship MUST NOT
automatically prompt the captain solely because the mate finished; the note is
delivered on the captain's next explicit prompt.

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
The mate agent MUST NOT be configured with raw ACP filesystem or terminal
capabilities. File access, search, editing, planning, and command execution
MUST flow through Ship's mate MCP tools instead of ACP built-ins.

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
Each project's `.ship/` directory (for session persistence, Ship-managed
worktrees, and MCP config) is relative to that project's repository root.
Ship's own configuration (project list, global settings) lives in
`~/.config/ship/`.

r[project.mcp-defaults]
Each project MAY have a `.ship/mcp-servers.json` in its repository root for
project-specific MCP server defaults. A global default MAY also be configured
in `~/.config/ship/mcp-servers.json`. Project-level config takes precedence.

## Composer

r[ui.composer.file-mention]
The composer MUST support @-triggered file autocomplete. Typing @ opens a
dropdown of worktree files filtered by the text after @. Selecting a file
inserts the path. On submit, the backend expands @path mentions by injecting
the referenced file contents into the prompt.

r[ui.composer.image-attach]
The composer MUST support image attachment via drag-and-drop onto the composer
area, clipboard paste (when the clipboard contains an image), and an attach
button. Attached images are displayed as thumbnails before sending and can be
removed individually. On submit, images are sent as vision content alongside
text. The backend encodes images as base64 and passes them to the ACP agent
as image content blocks.

## Human Review Panel

r[ui.human-review.panel]
When a `HumanReviewRequested` event is received, the UI MUST display a
blocking review panel showing the captain's message, a colored unified diff of
changes since the base branch, the worktree path (with a copy-to-clipboard
button), and two actions: Approve (sends `"approved"` via `reply_to_human`)
and Request Changes (opens a text field to enter feedback, then sends that
text via `reply_to_human`). The panel is dismissed when `HumanReviewCleared`
is received.

## Session Titles

r[feature.auto-title]
When the user sends their first message to a session, Ship MUST automatically
generate a short title (4–7 words) by prompting a background summarizer agent
with the user's message text. The summarizer is spawned fresh per session with
no MCP tools. On success, a `SessionTitleChanged` event is emitted. The
generated title is shown in the session sidebar in place of the branch name.

r[event.session-title-changed]
The `SessionTitleChanged { title: String }` event updates the session's title
in all subscribers. The backend stores the title as part of the session event
log so it is restored on server restart.

## Cost Tracking

r[cost.not-tracked]
Ship intentionally does NOT track token usage or API costs. Both Claude and
Codex are expected to be used via subscriptions (Claude Pro/Team, Codex
subscription), not metered API tokens. If a future agent kind requires API
billing, cost tracking can be added then.
