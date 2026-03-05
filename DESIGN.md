# Ship

Pair programming with AI agents. A captain steers, a mate builds.

## Concept

Ship coordinates two AI coding agents working together on a shared codebase.
One plays captain (architecture, review, direction), one plays mate (writes
code, runs tests, implements). The human watches, intervenes when needed, and
approves actions.

Built on ACP (Agent Client Protocol) for direct structured communication with
Claude Code and OpenAI Codex. Web-based UI for visibility and control.

## Sessions

A session is a pairing: one captain agent and one mate agent collaborating on
a branch. Each session has:

- A **captain** — Claude or Codex, user's choice
- A **mate** — Claude or Codex, user's choice
- A **git worktree** — isolated branch for the session's work
- A **task** — one active task at a time, plus history of completed tasks

Sessions are persistent across browser reloads and server restarts.

## Agent communication

Agents are controlled via ACP, which provides:

- **Prompts** — send structured instructions to an agent (`SessionPrompt`)
- **Notifications** — receive state changes as they happen
- **Stop reasons** — know exactly why an agent paused (done, tool use, token limit)
- **Plans** — the agent's execution plan with per-step status
- **Content blocks** — text, tool calls, images, diffs as typed data
- **Permissions** — approve or deny agent actions from the UI
- **Terminals** — managed command execution with exit codes

No terminal scraping. No polling. No heuristics. The agent tells you its state.

## Architecture

```
Browser (TypeScript)
    |
    | roam-websocket (codegen'd types)
    |
    v
Backend (Rust)
    |
    |--- ACP ---> Claude Code
    |--- ACP ---> Codex
    |--- git ---> worktree management
    |--- fs  ---> task persistence
```

### Backend

Rust server. Roam for frontend-backend RPC. Responsibilities:

- Agent lifecycle — spawn, connect via ACP, teardown
- Session state — active task, history, agent assignments
- Message routing — translates between the Ship protocol and ACP calls
- Git worktree creation and cleanup
- Task persistence — survives restarts

### Frontend

TypeScript. Types generated from the backend's Rust traits via roam-codegen.

Views:
- **Session list** — all sessions, agent states, current tasks at a glance
- **Session view** — captain and mate panels side by side, task controls
- **Agent panel** — state, context usage, execution plan, current activity
- **Task panel** — active task description, update history, steer/accept/cancel
- **Permission dialog** — approve or deny agent actions inline

No terminal emulator in the UI. Content blocks (code, diffs, text) are
rendered directly as structured elements.

## Protocol

The Ship RPC defines frontend-to-backend operations:

```rust
pub struct SessionId(Uuid);
pub struct TaskId(Uuid);

pub enum AgentKind {
    Claude,
    Codex,
}

pub enum Role {
    Captain,
    Mate,
}

#[roam::service]
trait Ship {
    /// Create a new session with the given agent configuration.
    async fn create_session(&self, req: CreateSessionRequest) -> Result<SessionId, ShipError>;

    /// List all active sessions.
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, ShipError>;

    /// Captain assigns a task to the mate.
    async fn assign(&self, req: AssignRequest) -> Result<TaskId, ShipError>;

    /// Captain steers the mate with feedback or new direction.
    async fn steer(&self, req: SteerRequest) -> Result<(), ShipError>;

    /// Captain accepts the mate's work and closes the task.
    async fn accept(&self, req: AcceptRequest) -> Result<(), ShipError>;

    /// Captain cancels the current task.
    async fn cancel(&self, req: CancelRequest) -> Result<(), ShipError>;

    /// Respond to a permission request from an agent.
    async fn resolve_permission(&self, req: ResolvePermissionRequest) -> Result<(), ShipError>;
}
```

The backend translates these into ACP operations against the actual agents.
Updates, progress, and state changes flow back to the frontend via a
subscription stream (roam bidirectional channel).

## Agent state

Derived from ACP events, not inference:

```rust
pub enum AgentState {
    /// Processing a prompt.
    Working {
        plan: Option<Vec<PlanStep>>,
        activity: Option<String>,
    },
    /// Finished, waiting for input.
    Idle,
    /// Waiting for human to approve/deny an action.
    AwaitingPermission {
        request: PermissionRequest,
    },
    /// Hit context window limit.
    ContextExhausted,
}

pub struct PlanStep {
    pub description: String,
    pub status: PlanStepStatus,
}

pub enum PlanStepStatus {
    Planned,
    InProgress,
    Completed,
    Failed,
}

pub struct AgentSnapshot {
    pub role: Role,
    pub kind: AgentKind,
    pub state: AgentState,
    /// Percentage of context window remaining (0-100).
    pub context_remaining_percent: Option<u8>,
}
```

## Worktrees

Each session operates in an isolated git worktree:

```
repo/
  .git/
  ...
  .worktrees/
    ship-{session_id}/
```

- Created from a user-specified base branch when the session starts
- Both agents operate within the worktree
- The human can merge the branch when satisfied
- Cleaned up on session close (with confirmation)

## Task lifecycle

```
Assign --> Working --> [Agent updates flow to captain] --> Steer
               |                                            |
               v                                            v
           Respond --> [Captain reviews] ---------> Accept
               |                                      |
               +-----------> Cancel <-----------------+
```

1. Human (or captain) sends `assign` with a task description
2. Backend sends a `SessionPrompt` to the mate via ACP
3. Mate works — backend receives ACP notifications, tool calls, plan updates
4. Backend streams progress to the frontend in real time
5. When the mate finishes (`StopReason::EndTurn`), captain reviews the output
6. Captain sends `steer` (more work needed) or human clicks `accept` (done)
7. On accept, the task moves to history; session is ready for the next task

## Event stream

The frontend subscribes to a session's event stream:

```rust
pub enum SessionEvent {
    /// Agent state changed.
    AgentStateChanged {
        role: Role,
        state: AgentState,
    },
    /// Agent produced content (text, tool call, diff, etc.).
    ContentBlock {
        role: Role,
        block: ContentBlock,
    },
    /// Agent is requesting permission for an action.
    PermissionRequested {
        role: Role,
        request: PermissionRequest,
    },
    /// Task status changed.
    TaskStatusChanged {
        task_id: TaskId,
        status: TaskStatus,
    },
    /// Context usage updated.
    ContextUpdated {
        role: Role,
        remaining_percent: u8,
    },
}
```

## Resilience

ACP sessions are persistent on the agent side. If the connection drops
mid-task, the backend reconnects and calls `LoadSession` to resume. Task state
lives in the backend, not the agent — so nothing is lost. If the agent lost
context (process crash), the backend injects a summary prompt to catch it up.

## Session sharing

Multiple browsers can watch the same session. Roam supports multiple
subscribers on the same event stream — every connected client receives the
same `SessionEvent`s. Steering is single-writer: one active controller per
session.

## Cost tracking

ACP exposes token usage. Surface it in the UI per-session, per-task,
per-agent. Running totals and per-prompt costs.

## Context exhaustion

When an agent's context window drops below 20%, the UI warns the human.
Future: auto-summarize the session, spawn a fresh agent, and inject the
summary as the opening prompt. For now, the human decides when to rotate.

## Autonomy modes

Two modes, togglable per session:

- **Human-in-the-loop** (default) — captain proposes steers, human approves
  before they're sent to the mate.
- **Autonomous** — captain auto-steers the mate. Human watches the event
  stream and can intervene at any time. The permission system still gates
  destructive actions regardless of mode.
