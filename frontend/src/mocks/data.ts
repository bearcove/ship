import type { AgentDiscovery, SessionScenarioKey, SessionListScenarioKey } from "../types";
import type {
  AgentSnapshot,
  ContentBlock,
  ProjectInfo,
  SessionDetail,
  SessionSummary,
  TaskRecord,
} from "../generated/ship";

function mkAgent(
  role: "Captain" | "Mate",
  kind: "Claude" | "Codex",
  state:
    | { tag: "Idle" }
    | { tag: "Working" }
    | { tag: "AwaitingPermission" }
    | { tag: "ContextExhausted" }
    | { tag: "Error"; message: string },
  contextRemainingPercent: number | null = null,
): AgentSnapshot {
  const agentState =
    state.tag === "Working"
      ? { tag: "Working" as const, plan: null, activity: null }
      : state.tag === "AwaitingPermission"
        ? {
            tag: "AwaitingPermission" as const,
            request: {
              permission_id: "perm_live_01",
              tool_name: "Bash",
              arguments: JSON.stringify({ command: "cargo test" }),
              description: "Run tests",
            },
          }
        : state;
  return {
    role: { tag: role },
    kind: { tag: kind },
    state: agentState,
    context_remaining_percent: contextRemainingPercent,
  };
}

export const MOCK_PROJECTS: ProjectInfo[] = [
  {
    name: "roam",
    path: "/home/user/bearcove/roam",
    valid: true,
    invalid_reason: null,
  },
  {
    name: "styx",
    path: "/home/user/bearcove/styx",
    valid: true,
    invalid_reason: null,
  },
  {
    name: "old-thing",
    path: "/home/user/bearcove/old-thing",
    valid: false,
    invalid_reason: "Path does not exist",
  },
];

export const MOCK_SESSIONS: SessionSummary[] = [
  {
    id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1A",
    project: "roam",
    branch_name: "feat/websocket-transport",
    captain: mkAgent("Captain", "Claude", { tag: "Working" }, 77),
    mate: mkAgent("Mate", "Claude", { tag: "Working" }, 94),
    current_task_description:
      "Implement WebSocket transport layer with reconnection logic and backoff",
    task_status: { tag: "Working" },
    autonomy_mode: { tag: "HumanInTheLoop" },
  },
  {
    id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1B",
    project: "roam",
    branch_name: "fix/session-handshake",
    captain: mkAgent("Captain", "Claude", { tag: "Idle" }, 89),
    mate: mkAgent("Mate", "Codex", { tag: "Idle" }, 82),
    current_task_description: "Fix handshake timeout when server sends large initial payload",
    task_status: { tag: "ReviewPending" },
    autonomy_mode: { tag: "HumanInTheLoop" },
  },
  {
    id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1C",
    project: "styx",
    branch_name: "feat/lsp-hover",
    captain: mkAgent("Captain", "Codex", { tag: "Idle" }, 60),
    mate: mkAgent("Mate", "Claude", { tag: "Idle" }, 70),
    current_task_description: "Add hover documentation support to the Styx language server",
    task_status: { tag: "SteerPending" },
    autonomy_mode: { tag: "HumanInTheLoop" },
  },
  {
    id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1D",
    project: "styx",
    branch_name: "refactor/parser-cleanup",
    captain: mkAgent("Captain", "Claude", { tag: "Idle" }, 55),
    mate: mkAgent("Mate", "Claude", { tag: "Idle" }, 65),
    current_task_description: "Refactor CST parser to reduce allocations in hot path",
    task_status: { tag: "Accepted" },
    autonomy_mode: { tag: "HumanInTheLoop" },
  },
];

export const MOCK_BRANCHES: Record<string, string[]> = {
  roam: [
    "main",
    "develop",
    "feat/websocket-transport",
    "feat/shared-memory-ipc",
    "feat/codegen-typescript",
    "feat/opentelemetry-tracing",
    "fix/session-handshake",
    "fix/reconnect-race",
    "fix/memory-leak-ring-buffer",
    "chore/update-deps",
  ],
  styx: [
    "main",
    "develop",
    "feat/lsp-hover",
    "feat/lsp-goto-definition",
    "feat/wasm-bindings",
    "fix/norway-problem",
    "refactor/parser-cleanup",
    "refactor/session-state-machine",
    "docs/api-reference",
    "experiment/cranelift-jit",
  ],
};

export const MOCK_AGENT_DISCOVERY: AgentDiscovery = {
  claude: true,
  codex: false,
};

// ─── Session Detail ───────────────────────────────────────────────────────────

export const MOCK_SESSION_DETAIL: SessionDetail = {
  id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1A",
  project: "roam",
  branch_name: "feat/websocket-transport",
  autonomy_mode: { tag: "HumanInTheLoop" },
  captain: mkAgent("Captain", "Claude", { tag: "Working" }, 77),
  mate: mkAgent("Mate", "Claude", { tag: "AwaitingPermission" }, 94),
  current_task: {
    id: "task_01",
    description: "Implement WebSocket transport layer with reconnection logic and backoff",
    status: { tag: "Working" },
  },
  task_history: [],
  pending_steer:
    "The mate should implement the `reconnect` method using **exponential backoff** starting at 100ms, capping at 30s. Use the existing `BackoffConfig` struct.\n\nKey steps:\n1. Track attempt count in session state\n2. Compute delay: `min(base * 2^attempt, max_delay)`\n3. Reset counter on successful connection",
};

// ─── Session Events ───────────────────────────────────────────────────────────

export const MOCK_CAPTAIN_EVENTS: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Analyze existing transport abstractions", status: { tag: "Completed" } },
      { description: "Design reconnection state machine", status: { tag: "Completed" } },
      {
        description: "Draft steer for mate: implement reconnect logic",
        status: { tag: "InProgress" },
      },
      { description: "Review mate's implementation", status: { tag: "Planned" } },
      { description: "Write integration tests", status: { tag: "Planned" } },
    ],
  },
  {
    tag: "Text",
    text: "I'll start by reading the existing transport code to understand the current structure before designing the reconnection approach.",
  },
  {
    tag: "ToolCall",
    tool_name: "Read",
    arguments: JSON.stringify({ path: "crates/roam-websocket/src/transport.rs" }),
    status: { tag: "Success" },
    result:
      "pub struct WebSocketTransport {\n    url: String,\n    conn: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,\n}\n\nimpl WebSocketTransport {\n    pub async fn connect(url: &str) -> Result<Self> { ... }\n    pub async fn send(&mut self, msg: Message) -> Result<()> { ... }\n    pub async fn recv(&mut self) -> Result<Message> { ... }\n}",
  },
  {
    tag: "ToolCall",
    tool_name: "Read",
    arguments: JSON.stringify({ path: "crates/roam-session/src/config.rs" }),
    status: { tag: "Success" },
    result:
      "pub struct BackoffConfig {\n    pub base_ms: u64,\n    pub max_ms: u64,\n    pub multiplier: f64,\n}",
  },
  {
    tag: "Text",
    text: "Good — `BackoffConfig` already exists. The mate just needs to add `reconnect_attempts: u32` to session state and wire up the delay computation. I'll draft the steer now.",
  },
  {
    tag: "ToolCall",
    tool_name: "Grep",
    arguments: JSON.stringify({ pattern: "reconnect", path: "crates/" }),
    status: { tag: "Success" },
    result:
      "crates/roam-session/src/session.rs:14:    // TODO: implement reconnection\ncrates/roam-websocket/src/transport.rs:87:    // reconnect not implemented",
  },
];

export const MOCK_MATE_EVENTS: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Read session state struct", status: { tag: "Completed" } },
      { description: "Add reconnect_attempts field", status: { tag: "Completed" } },
      { description: "Implement backoff delay computation", status: { tag: "Completed" } },
      { description: "Implement reconnect method", status: { tag: "InProgress" } },
      { description: "Update connect() to use reconnect on failure", status: { tag: "Planned" } },
    ],
  },
  {
    tag: "Text",
    text: "I've read the captain's steer. I'll add the `reconnect_attempts` counter to `SessionState` and implement the exponential backoff logic.",
  },
  {
    tag: "ToolCall",
    tool_name: "Edit",
    arguments: JSON.stringify({ path: "crates/roam-session/src/session.rs" }),
    status: { tag: "Success" },
    result:
      "--- a/crates/roam-session/src/session.rs\n+++ b/crates/roam-session/src/session.rs\n@@ -1,3 +1,4 @@\n pub struct SessionState {\n     pub connected: bool,\n+    pub reconnect_attempts: u32,\n }",
  },
  {
    tag: "ToolCall",
    tool_name: "Bash",
    arguments: JSON.stringify({ command: "cargo check -p roam-session 2>&1" }),
    status: { tag: "Failure" },
    result:
      "error[E0063]: missing field `reconnect_attempts` in initializer of `SessionState`\n  --> crates/roam-session/src/session.rs:42:18\n   |\n42 |         let state = SessionState { connected: false };\n   |                     ^^^^^^^^^^^^ missing `reconnect_attempts`\n\nerror: could not compile `roam-session`",
  },
  {
    tag: "Error",
    message:
      "Compilation failed after adding `reconnect_attempts`. Need to update all `SessionState` constructors to include the new field.",
  },
  {
    tag: "Permission",
    tool_name: "Bash",
    description: "Run `cargo test -p roam-session` to verify the fix",
    arguments: JSON.stringify({ command: "cargo test -p roam-session 2>&1" }),
    resolution: null,
  },
];

// ─── Task History ─────────────────────────────────────────────────────────────

export const MOCK_TASK_HISTORY: TaskRecord[] = [
  {
    id: "task_hist_01",
    description: "Set up WebSocket transport crate skeleton with basic send/recv",
    status: { tag: "Accepted" },
  },
  {
    id: "task_hist_02",
    description: "Add TLS support to WebSocket transport via tokio-rustls",
    status: { tag: "Accepted" },
  },
];

// ─── Additional Scenario Data ─────────────────────────────────────────────────

const CAPTAIN_EVENTS_IDLE: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Analyze LSP protocol requirements for hover", status: { tag: "Completed" } },
      { description: "Review existing request handlers", status: { tag: "Completed" } },
      { description: "Draft steer for mate", status: { tag: "Completed" } },
      { description: "Review mate's implementation", status: { tag: "Planned" } },
    ],
  },
  {
    tag: "Text",
    text: "Steer sent to the mate. Waiting for hover handler implementation.",
  },
];

const MATE_EVENTS_WORKING: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Read LSP request handler structure", status: { tag: "Completed" } },
      { description: "Implement textDocument/hover handler", status: { tag: "InProgress" } },
      { description: "Add hover result formatting", status: { tag: "Planned" } },
      { description: "Write tests", status: { tag: "Planned" } },
    ],
  },
  {
    tag: "Text",
    text: "Reading the existing request handler infrastructure to understand the pattern.",
  },
  {
    tag: "ToolCall",
    tool_name: "Read",
    arguments: JSON.stringify({ path: "crates/styx-lsp/src/handler.rs" }),
    status: { tag: "Success" },
    result: "pub trait RequestHandler {\n    async fn handle(&self, req: Request) -> Response;\n}",
  },
  {
    tag: "ToolCall",
    tool_name: "Write",
    arguments: JSON.stringify({ path: "crates/styx-lsp/src/handlers/hover.rs" }),
    status: { tag: "Success" },
    result: "Created new file with hover handler skeleton",
  },
];

const MATE_EVENTS_AWAITING_PERMISSION: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Design ring buffer layout", status: { tag: "Completed" } },
      { description: "Implement writer side", status: { tag: "Completed" } },
      { description: "Implement reader side", status: { tag: "Completed" } },
      { description: "Run integration tests", status: { tag: "InProgress" } },
    ],
  },
  {
    tag: "Text",
    text: "Implementation complete. Running the full test suite to verify correctness.",
  },
  {
    tag: "ToolCall",
    tool_name: "Edit",
    arguments: JSON.stringify({ path: "crates/roam-shm/src/ring.rs" }),
    status: { tag: "Success" },
    result: "Ring buffer implemented",
  },
  {
    tag: "Permission",
    tool_name: "Bash",
    description: "Run `cargo test -p roam-shm` to verify ring buffer implementation",
    arguments: JSON.stringify({ command: "cargo test -p roam-shm 2>&1" }),
    resolution: null,
  },
];

const CAPTAIN_EVENTS_ERROR: ContentBlock[] = [
  {
    tag: "Error",
    message:
      "ACP connection lost: process exited with code 1. Check that `claude-agent-acp` is installed and on PATH.",
  },
];

const MATE_EVENTS_IDLE_WAITING: ContentBlock[] = [
  {
    tag: "Text",
    text: "Waiting for the captain to provide direction on the refactoring approach.",
  },
];

const MATE_EVENTS_CONTEXT_EXHAUSTED: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Audit all RPC call sites", status: { tag: "Completed" } },
      { description: "Add span instrumentation to roam-websocket", status: { tag: "Completed" } },
      { description: "Add span instrumentation to roam-session", status: { tag: "Completed" } },
      { description: "Add span instrumentation to roam-shm", status: { tag: "Completed" } },
      { description: "Add context propagation headers", status: { tag: "Completed" } },
      { description: "Write integration test", status: { tag: "InProgress" } },
    ],
  },
  {
    tag: "ToolCall",
    tool_name: "Edit",
    arguments: JSON.stringify({ path: "crates/roam-websocket/src/transport.rs" }),
    status: { tag: "Success" },
    result: "Added tracing spans",
  },
  {
    tag: "ToolCall",
    tool_name: "Edit",
    arguments: JSON.stringify({ path: "crates/roam-session/src/session.rs" }),
    status: { tag: "Success" },
    result: "Added tracing spans",
  },
  {
    tag: "ToolCall",
    tool_name: "Edit",
    arguments: JSON.stringify({ path: "crates/roam-shm/src/ring.rs" }),
    status: { tag: "Success" },
    result: "Added tracing spans",
  },
  {
    tag: "ToolCall",
    tool_name: "Bash",
    arguments: JSON.stringify({ command: "cargo check --workspace 2>&1" }),
    status: { tag: "Success" },
    result: "warning: unused import: `tracing::instrument`\n  Finished checking 12 targets",
  },
  {
    tag: "ToolCall",
    tool_name: "Bash",
    arguments: JSON.stringify({ command: "cargo test -p roam-session 2>&1" }),
    status: { tag: "Success" },
    result: "test result: ok. 8 passed; 0 failed",
  },
  {
    tag: "Error",
    message: "Context window nearing limit. Unable to continue — rotate agent to proceed.",
  },
];

const CAPTAIN_EVENTS_STEER_PENDING: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Read the reconnect implementation", status: { tag: "Completed" } },
      { description: "Identify root cause of race", status: { tag: "Completed" } },
      { description: "Draft steer for mate", status: { tag: "Completed" } },
      { description: "Review fix implementation", status: { tag: "Planned" } },
    ],
  },
  {
    tag: "Text",
    text: "Found the race condition. Steer is ready for your review — approve to send to the mate.",
  },
];

const MATE_EVENTS_STEER_PENDING: ContentBlock[] = [
  {
    tag: "Text",
    text: "Hit a blocker with the reconnect handler. Waiting for captain's guidance.",
  },
];

const CAPTAIN_EVENTS_AUTONOMOUS: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Review existing codegen output format", status: { tag: "Completed" } },
      { description: "Draft TypeScript client template", status: { tag: "InProgress" } },
      { description: "Send steer to mate", status: { tag: "Planned" } },
      { description: "Review generated code", status: { tag: "Planned" } },
    ],
  },
  {
    tag: "Text",
    text: "Drafting the TypeScript client structure. In autonomous mode, steer goes directly to the mate without human review.",
  },
];

const MATE_EVENTS_AUTONOMOUS: ContentBlock[] = [
  {
    tag: "PlanUpdate",
    steps: [
      { description: "Read roam service trait definitions", status: { tag: "Completed" } },
      { description: "Parse codegen templates", status: { tag: "InProgress" } },
      { description: "Generate TypeScript types", status: { tag: "Planned" } },
      { description: "Generate client methods", status: { tag: "Planned" } },
    ],
  },
  {
    tag: "ToolCall",
    tool_name: "Read",
    arguments: JSON.stringify({ path: "crates/roam-codegen/src/typescript.rs" }),
    status: { tag: "Success" },
    result:
      "// Codegen template for TypeScript\npub fn generate_client(service: &ServiceDef) -> String { ... }",
  },
];

// ─── Scenario Maps ────────────────────────────────────────────────────────────

export interface SessionScenario {
  detail: SessionDetail;
  captainEvents: ContentBlock[];
  mateEvents: ContentBlock[];
  taskHistory: TaskRecord[];
}

export const SESSION_SCENARIOS: Record<SessionScenarioKey, SessionScenario> = {
  "happy-path": {
    detail: MOCK_SESSION_DETAIL,
    captainEvents: MOCK_CAPTAIN_EVENTS,
    mateEvents: MOCK_MATE_EVENTS,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "captain-idle-mate-working": {
    detail: {
      id: "sess_cimw",
      project: "styx",
      branch_name: "feat/lsp-hover",
      autonomy_mode: { tag: "HumanInTheLoop" },
      captain: mkAgent("Captain", "Claude", { tag: "Idle" }, 96),
      mate: mkAgent("Mate", "Codex", { tag: "Working" }, 82),
      current_task: {
        id: "task_lsp_01",
        description: "Add hover documentation support to the Styx language server",
        status: { tag: "Working" },
      },
      task_history: [],
      pending_steer: null,
    },
    captainEvents: CAPTAIN_EVENTS_IDLE,
    mateEvents: MATE_EVENTS_WORKING,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "mate-awaiting-permission": {
    detail: {
      id: "sess_map",
      project: "roam",
      branch_name: "feat/shared-memory-ipc",
      autonomy_mode: { tag: "HumanInTheLoop" },
      captain: mkAgent("Captain", "Claude", { tag: "Working" }, 89),
      mate: mkAgent("Mate", "Claude", { tag: "AwaitingPermission" }, 91),
      current_task: {
        id: "task_shm_01",
        description: "Implement lock-free shared memory ring buffer for IPC",
        status: { tag: "Working" },
      },
      task_history: [],
      pending_steer: null,
    },
    captainEvents: CAPTAIN_EVENTS_IDLE,
    mateEvents: MATE_EVENTS_AWAITING_PERMISSION,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "agent-error": {
    detail: {
      id: "sess_err",
      project: "styx",
      branch_name: "refactor/parser-cleanup",
      autonomy_mode: { tag: "HumanInTheLoop" },
      captain: mkAgent("Captain", "Claude", {
        tag: "Error",
        message:
          "ACP connection lost: process exited with code 1. Check that `claude-agent-acp` is installed and on PATH.",
      }),
      mate: mkAgent("Mate", "Claude", { tag: "Idle" }, 97),
      current_task: {
        id: "task_ref_01",
        description: "Refactor CST parser to reduce allocations in hot path",
        status: { tag: "Working" },
      },
      task_history: [],
      pending_steer: null,
    },
    captainEvents: CAPTAIN_EVENTS_ERROR,
    mateEvents: MATE_EVENTS_IDLE_WAITING,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "context-exhausted": {
    detail: {
      id: "sess_ctx",
      project: "roam",
      branch_name: "feat/opentelemetry-tracing",
      autonomy_mode: { tag: "HumanInTheLoop" },
      captain: mkAgent("Captain", "Claude", { tag: "Working" }, 80),
      mate: mkAgent("Mate", "Claude", { tag: "ContextExhausted" }, 1),
      current_task: {
        id: "task_otel_01",
        description: "Add OpenTelemetry instrumentation to all RPC call sites",
        status: { tag: "Working" },
      },
      task_history: [],
      pending_steer: null,
    },
    captainEvents: MOCK_CAPTAIN_EVENTS,
    mateEvents: MATE_EVENTS_CONTEXT_EXHAUSTED,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "steer-pending": {
    detail: {
      id: "sess_steer",
      project: "roam",
      branch_name: "fix/reconnect-race",
      autonomy_mode: { tag: "HumanInTheLoop" },
      captain: mkAgent("Captain", "Claude", { tag: "Idle" }, 84),
      mate: mkAgent("Mate", "Claude", { tag: "Idle" }, 92),
      current_task: {
        id: "task_race_01",
        description: "Fix race condition in WebSocket reconnect handler",
        status: { tag: "SteerPending" },
      },
      task_history: [],
      pending_steer:
        "The race condition is in `reconnect()` — it modifies `self.conn` without holding the lock. Fix:\n\n1. Wrap `self.conn` in an `Arc<Mutex<Option<Connection>>>`\n2. Acquire the lock before checking and setting `conn`\n3. Release the lock before the async connect call\n4. Re-acquire to store the new connection\n\nSee `session.rs:142` for the existing locking pattern.",
    },
    captainEvents: CAPTAIN_EVENTS_STEER_PENDING,
    mateEvents: MATE_EVENTS_STEER_PENDING,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "no-active-task": {
    detail: {
      id: "sess_notask",
      project: "styx",
      branch_name: "feat/wasm-bindings",
      autonomy_mode: { tag: "HumanInTheLoop" },
      captain: mkAgent("Captain", "Claude", { tag: "Idle" }),
      mate: mkAgent("Mate", "Claude", { tag: "Idle" }),
      current_task: null,
      task_history: [
        ...MOCK_TASK_HISTORY,
        {
          id: "task_hist_03",
          description: "Implement WASM bindings for Styx schema validation",
          status: { tag: "Accepted" },
        },
      ],
      pending_steer: null,
    },
    captainEvents: [],
    mateEvents: [],
    taskHistory: [
      ...MOCK_TASK_HISTORY,
      {
        id: "task_hist_03",
        description: "Implement WASM bindings for Styx schema validation",
        status: { tag: "Accepted" },
      },
    ],
  },
  "autonomous-mode": {
    detail: {
      id: "sess_auto",
      project: "roam",
      branch_name: "feat/codegen-typescript",
      autonomy_mode: { tag: "Autonomous" },
      captain: mkAgent("Captain", "Claude", { tag: "Working" }, 94),
      mate: mkAgent("Mate", "Claude", { tag: "Working" }, 86),
      current_task: {
        id: "task_codegen_01",
        description: "Generate TypeScript client from roam service definitions",
        status: { tag: "Working" },
      },
      task_history: [],
      pending_steer: null,
    },
    captainEvents: CAPTAIN_EVENTS_AUTONOMOUS,
    mateEvents: MATE_EVENTS_AUTONOMOUS,
    taskHistory: MOCK_TASK_HISTORY,
  },
};

export const SESSION_LIST_SCENARIOS: Record<SessionListScenarioKey, SessionSummary[]> = {
  normal: MOCK_SESSIONS,
  empty: [],
  "with-idle-reminders": [
    {
      id: "sess_idle_01",
      project: "roam",
      branch_name: "feat/websocket-transport",
      captain: mkAgent("Captain", "Claude", { tag: "Idle" }, 72),
      mate: mkAgent("Mate", "Claude", { tag: "Idle" }, 85),
      current_task_description:
        "Implement WebSocket transport layer with reconnection logic and backoff",
      task_status: { tag: "ReviewPending" },
      autonomy_mode: { tag: "HumanInTheLoop" },
    },
    {
      id: "sess_idle_02",
      project: "styx",
      branch_name: "feat/lsp-hover",
      captain: mkAgent("Captain", "Claude", { tag: "Idle" }, 60),
      mate: mkAgent("Mate", "Codex", { tag: "Idle" }, 75),
      current_task_description: "Add hover documentation support to the Styx language server",
      task_status: { tag: "SteerPending" },
      autonomy_mode: { tag: "HumanInTheLoop" },
    },
    {
      id: "sess_idle_03",
      project: "roam",
      branch_name: "fix/reconnect-race",
      captain: mkAgent("Captain", "Codex", { tag: "Working" }, 88),
      mate: mkAgent("Mate", "Claude", { tag: "Working" }, 91),
      current_task_description: "Fix race condition in WebSocket reconnect handler",
      task_status: { tag: "Working" },
      autonomy_mode: { tag: "HumanInTheLoop" },
    },
    {
      id: "sess_idle_04",
      project: "styx",
      branch_name: "refactor/parser-cleanup",
      captain: mkAgent("Captain", "Claude", { tag: "Idle" }, 55),
      mate: mkAgent("Mate", "Claude", { tag: "Idle" }, 63),
      current_task_description: "Refactor CST parser to reduce allocations in hot path",
      task_status: { tag: "ReviewPending" },
      autonomy_mode: { tag: "HumanInTheLoop" },
    },
  ],
};
