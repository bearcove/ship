import type {
  AgentDiscovery,
  ContentBlock,
  Project,
  SessionDetail,
  SessionListScenarioKey,
  SessionScenarioKey,
  SessionSummary,
  Task,
} from "../types";

export const MOCK_PROJECTS: Project[] = [
  {
    name: "roam",
    path: "/home/user/bearcove/roam",
    valid: true,
  },
  {
    name: "styx",
    path: "/home/user/bearcove/styx",
    valid: true,
  },
  {
    name: "old-thing",
    path: "/home/user/bearcove/old-thing",
    valid: false,
    invalidReason: "Path does not exist",
  },
];

const now = new Date();
const minutesAgo = (m: number) => new Date(now.getTime() - m * 60_000);

export const MOCK_SESSIONS: SessionSummary[] = [
  {
    id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1A",
    projectName: "roam",
    branch: "feat/websocket-transport",
    captainKind: "claude",
    mateKind: "claude",
    taskDescription: "Implement WebSocket transport layer with reconnection logic and backoff",
    taskStatus: "Working",
    lastActivityAt: minutesAgo(3),
    hasIdleReminder: false,
  },
  {
    id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1B",
    projectName: "roam",
    branch: "fix/session-handshake",
    captainKind: "claude",
    mateKind: "codex",
    taskDescription: "Fix handshake timeout when server sends large initial payload",
    taskStatus: "ReviewPending",
    lastActivityAt: minutesAgo(12),
    hasIdleReminder: true,
  },
  {
    id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1C",
    projectName: "styx",
    branch: "feat/lsp-hover",
    captainKind: "codex",
    mateKind: "claude",
    taskDescription: "Add hover documentation support to the Styx language server",
    taskStatus: "SteerPending",
    lastActivityAt: minutesAgo(28),
    hasIdleReminder: false,
  },
  {
    id: "sess_01HN8K2M3P4Q5R6S7T8U9V0W1D",
    projectName: "styx",
    branch: "refactor/parser-cleanup",
    captainKind: "claude",
    mateKind: "claude",
    taskDescription: "Refactor CST parser to reduce allocations in hot path",
    taskStatus: "Accepted",
    lastActivityAt: minutesAgo(74),
    hasIdleReminder: false,
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
  projectName: "roam",
  branch: "feat/websocket-transport",
  autonomyMode: "human-in-the-loop",
  captain: {
    kind: "claude",
    role: "captain",
    state: "working",
    context: { used: 45000, total: 200000 },
  },
  mate: {
    kind: "claude",
    role: "mate",
    state: "awaiting-permission",
    context: { used: 12000, total: 200000 },
  },
  activeTask: {
    id: "task_01",
    description: "Implement WebSocket transport layer with reconnection logic and backoff",
    status: "Working",
    createdAt: minutesAgo(45),
  },
  pendingSteer: {
    id: "steer_01",
    captainSteer:
      "The mate should implement the `reconnect` method using **exponential backoff** starting at 100ms, capping at 30s. Use the existing `BackoffConfig` struct.\n\nKey steps:\n1. Track attempt count in session state\n2. Compute delay: `min(base * 2^attempt, max_delay)`\n3. Reset counter on successful connection",
  },
};

// ─── Session Events ───────────────────────────────────────────────────────────

export const MOCK_CAPTAIN_EVENTS: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_01",
    role: "captain",
    steps: [
      { description: "Analyze existing transport abstractions", status: "completed" },
      { description: "Design reconnection state machine", status: "completed" },
      { description: "Draft steer for mate: implement reconnect logic", status: "in-progress" },
      { description: "Review mate's implementation", status: "planned" },
      { description: "Write integration tests", status: "planned" },
    ],
  },
  {
    type: "text",
    id: "txt_01",
    role: "captain",
    content:
      "I'll start by reading the existing transport code to understand the current structure before designing the reconnection approach.",
  },
  {
    type: "tool-call",
    id: "tool_01",
    toolName: "Read",
    kind: "read",
    filePath: "crates/roam-websocket/src/transport.rs",
    args: { path: "crates/roam-websocket/src/transport.rs" },
    result:
      "pub struct WebSocketTransport {\n    url: String,\n    conn: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,\n}\n\nimpl WebSocketTransport {\n    pub async fn connect(url: &str) -> Result<Self> { ... }\n    pub async fn send(&mut self, msg: Message) -> Result<()> { ... }\n    pub async fn recv(&mut self) -> Result<Message> { ... }\n}",
    status: "success",
  },
  {
    type: "tool-call",
    id: "tool_02",
    toolName: "Read",
    kind: "read",
    filePath: "crates/roam-session/src/config.rs",
    args: { path: "crates/roam-session/src/config.rs" },
    result:
      "pub struct BackoffConfig {\n    pub base_ms: u64,\n    pub max_ms: u64,\n    pub multiplier: f64,\n}",
    status: "success",
  },
  {
    type: "text",
    id: "txt_02",
    role: "captain",
    content:
      "Good — `BackoffConfig` already exists. The mate just needs to add `reconnect_attempts: u32` to session state and wire up the delay computation. I'll draft the steer now.",
  },
  {
    type: "tool-call",
    id: "tool_03",
    toolName: "Grep",
    kind: "search",
    query: "reconnect",
    args: { pattern: "reconnect", path: "crates/" },
    result:
      "crates/roam-session/src/session.rs:14:    // TODO: implement reconnection\ncrates/roam-websocket/src/transport.rs:87:    // reconnect not implemented",
    status: "success",
  },
];

export const MOCK_MATE_EVENTS: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_mate_01",
    role: "mate",
    steps: [
      { description: "Read session state struct", status: "completed" },
      { description: "Add reconnect_attempts field", status: "completed" },
      { description: "Implement backoff delay computation", status: "completed" },
      { description: "Implement reconnect method", status: "in-progress" },
      { description: "Update connect() to use reconnect on failure", status: "planned" },
    ],
  },
  {
    type: "text",
    id: "txt_mate_01",
    role: "mate",
    content:
      "I've read the captain's steer. I'll add the `reconnect_attempts` counter to `SessionState` and implement the exponential backoff logic.",
  },
  {
    type: "tool-call",
    id: "tool_mate_01",
    toolName: "Edit",
    kind: "edit",
    filePath: "crates/roam-session/src/session.rs",
    args: {
      path: "crates/roam-session/src/session.rs",
      old_string: "pub struct SessionState {\n    pub connected: bool,\n}",
      new_string:
        "pub struct SessionState {\n    pub connected: bool,\n    pub reconnect_attempts: u32,\n}",
    },
    result:
      "--- a/crates/roam-session/src/session.rs\n+++ b/crates/roam-session/src/session.rs\n@@ -1,3 +1,4 @@\n pub struct SessionState {\n     pub connected: bool,\n+    pub reconnect_attempts: u32,\n }",
    status: "success",
    diffSummary: "+1 -0",
  },
  {
    type: "tool-call",
    id: "tool_mate_02",
    toolName: "Bash",
    kind: "command",
    command: "cargo check -p roam-session 2>&1",
    args: { command: "cargo check -p roam-session 2>&1" },
    result:
      "error[E0063]: missing field `reconnect_attempts` in initializer of `SessionState`\n  --> crates/roam-session/src/session.rs:42:18\n   |\n42 |         let state = SessionState { connected: false };\n   |                     ^^^^^^^^^^^^ missing `reconnect_attempts`\n\nerror: could not compile `roam-session`",
    status: "failure",
  },
  {
    type: "error",
    id: "err_mate_01",
    role: "mate",
    message:
      "Compilation failed after adding `reconnect_attempts`. Need to update all `SessionState` constructors to include the new field.",
  },
  {
    type: "permission",
    id: "perm_01",
    role: "mate",
    toolName: "Bash",
    description: "Run `cargo test -p roam-session` to verify the fix",
    args: { command: "cargo test -p roam-session 2>&1" },
  },
];

// ─── Task History ─────────────────────────────────────────────────────────────

export const MOCK_TASK_HISTORY: Task[] = [
  {
    id: "task_hist_01",
    description: "Set up WebSocket transport crate skeleton with basic send/recv",
    status: "Accepted",
    createdAt: minutesAgo(180),
    completedAt: minutesAgo(120),
  },
  {
    id: "task_hist_02",
    description: "Add TLS support to WebSocket transport via tokio-rustls",
    status: "Accepted",
    createdAt: minutesAgo(115),
    completedAt: minutesAgo(60),
  },
];

// ─── Additional Scenario Data ─────────────────────────────────────────────────

const CAPTAIN_EVENTS_IDLE: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_ci_01",
    role: "captain",
    steps: [
      { description: "Analyze LSP protocol requirements for hover", status: "completed" },
      { description: "Review existing request handlers", status: "completed" },
      { description: "Draft steer for mate", status: "completed" },
      { description: "Review mate's implementation", status: "planned" },
    ],
  },
  {
    type: "text",
    id: "txt_ci_01",
    role: "captain",
    content: "Steer sent to the mate. Waiting for hover handler implementation.",
  },
];

const MATE_EVENTS_WORKING: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_mw_01",
    role: "mate",
    steps: [
      { description: "Read LSP request handler structure", status: "completed" },
      { description: "Implement textDocument/hover handler", status: "in-progress" },
      { description: "Add hover result formatting", status: "planned" },
      { description: "Write tests", status: "planned" },
    ],
  },
  {
    type: "text",
    id: "txt_mw_01",
    role: "mate",
    content: "Reading the existing request handler infrastructure to understand the pattern.",
  },
  {
    type: "tool-call",
    id: "tool_mw_01",
    toolName: "Read",
    kind: "read",
    filePath: "crates/styx-lsp/src/handler.rs",
    args: { path: "crates/styx-lsp/src/handler.rs" },
    result: "pub trait RequestHandler {\n    async fn handle(&self, req: Request) -> Response;\n}",
    status: "success",
  },
  {
    type: "tool-call",
    id: "tool_mw_02",
    toolName: "Write",
    kind: "write",
    filePath: "crates/styx-lsp/src/handlers/hover.rs",
    args: { path: "crates/styx-lsp/src/handlers/hover.rs" },
    result: "Created new file with hover handler skeleton",
    status: "success",
    diffSummary: "+45 -0",
  },
];

const MATE_EVENTS_AWAITING_PERMISSION: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_ap_01",
    role: "mate",
    steps: [
      { description: "Design ring buffer layout", status: "completed" },
      { description: "Implement writer side", status: "completed" },
      { description: "Implement reader side", status: "completed" },
      { description: "Run integration tests", status: "in-progress" },
    ],
  },
  {
    type: "text",
    id: "txt_ap_01",
    role: "mate",
    content: "Implementation complete. Running the full test suite to verify correctness.",
  },
  {
    type: "tool-call",
    id: "tool_ap_01",
    toolName: "Edit",
    kind: "edit",
    filePath: "crates/roam-shm/src/ring.rs",
    args: {},
    result: "Ring buffer implemented",
    status: "success",
    diffSummary: "+120 -8",
  },
  {
    type: "permission",
    id: "perm_live_01",
    role: "mate",
    toolName: "Bash",
    description: "Run `cargo test -p roam-shm` to verify ring buffer implementation",
    args: { command: "cargo test -p roam-shm 2>&1" },
  },
];

const CAPTAIN_EVENTS_ERROR: ContentBlock[] = [
  {
    type: "error",
    id: "err_cap_01",
    role: "captain",
    message:
      "ACP connection lost: process exited with code 1. Check that `claude-agent-acp` is installed and on PATH.",
  },
];

const MATE_EVENTS_IDLE_WAITING: ContentBlock[] = [
  {
    type: "text",
    id: "txt_idle_mate_01",
    role: "mate",
    content: "Waiting for the captain to provide direction on the refactoring approach.",
  },
];

const MATE_EVENTS_CONTEXT_EXHAUSTED: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_ce_01",
    role: "mate",
    steps: [
      { description: "Audit all RPC call sites", status: "completed" },
      { description: "Add span instrumentation to roam-websocket", status: "completed" },
      { description: "Add span instrumentation to roam-session", status: "completed" },
      { description: "Add span instrumentation to roam-shm", status: "completed" },
      { description: "Add context propagation headers", status: "completed" },
      { description: "Write integration test", status: "in-progress" },
    ],
  },
  {
    type: "tool-call",
    id: "tool_ce_01",
    toolName: "Edit",
    kind: "edit",
    filePath: "crates/roam-websocket/src/transport.rs",
    args: {},
    result: "Added tracing spans",
    status: "success",
    diffSummary: "+34 -2",
  },
  {
    type: "tool-call",
    id: "tool_ce_02",
    toolName: "Edit",
    kind: "edit",
    filePath: "crates/roam-session/src/session.rs",
    args: {},
    result: "Added tracing spans",
    status: "success",
    diffSummary: "+28 -1",
  },
  {
    type: "tool-call",
    id: "tool_ce_03",
    toolName: "Edit",
    kind: "edit",
    filePath: "crates/roam-shm/src/ring.rs",
    args: {},
    result: "Added tracing spans",
    status: "success",
    diffSummary: "+19 -0",
  },
  {
    type: "tool-call",
    id: "tool_ce_04",
    toolName: "Bash",
    kind: "command",
    command: "cargo check --workspace 2>&1",
    args: { command: "cargo check --workspace 2>&1" },
    result: "warning: unused import: `tracing::instrument`\n  Finished checking 12 targets",
    status: "success",
  },
  {
    type: "tool-call",
    id: "tool_ce_05",
    toolName: "Bash",
    kind: "command",
    command: "cargo test -p roam-session 2>&1",
    args: {},
    result: "test result: ok. 8 passed; 0 failed",
    status: "success",
  },
  {
    type: "error",
    id: "err_ce_01",
    role: "mate",
    message: "Context window nearing limit. Unable to continue — rotate agent to proceed.",
  },
];

const CAPTAIN_EVENTS_STEER_PENDING: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_sp_01",
    role: "captain",
    steps: [
      { description: "Read the reconnect implementation", status: "completed" },
      { description: "Identify root cause of race", status: "completed" },
      { description: "Draft steer for mate", status: "completed" },
      { description: "Review fix implementation", status: "planned" },
    ],
  },
  {
    type: "text",
    id: "txt_sp_01",
    role: "captain",
    content:
      "Found the race condition. Steer is ready for your review — approve to send to the mate.",
  },
];

const MATE_EVENTS_STEER_PENDING: ContentBlock[] = [
  {
    type: "text",
    id: "txt_msp_01",
    role: "mate",
    content: "Hit a blocker with the reconnect handler. Waiting for captain's guidance.",
  },
];

const CAPTAIN_EVENTS_AUTONOMOUS: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_au_01",
    role: "captain",
    steps: [
      { description: "Review existing codegen output format", status: "completed" },
      { description: "Draft TypeScript client template", status: "in-progress" },
      { description: "Send steer to mate", status: "planned" },
      { description: "Review generated code", status: "planned" },
    ],
  },
  {
    type: "text",
    id: "txt_au_01",
    role: "captain",
    content:
      "Drafting the TypeScript client structure. In autonomous mode, steer goes directly to the mate without human review.",
  },
];

const MATE_EVENTS_AUTONOMOUS: ContentBlock[] = [
  {
    type: "plan-update",
    id: "plan_am_01",
    role: "mate",
    steps: [
      { description: "Read roam service trait definitions", status: "completed" },
      { description: "Parse codegen templates", status: "in-progress" },
      { description: "Generate TypeScript types", status: "planned" },
      { description: "Generate client methods", status: "planned" },
    ],
  },
  {
    type: "tool-call",
    id: "tool_am_01",
    toolName: "Read",
    kind: "read",
    filePath: "crates/roam-codegen/src/typescript.rs",
    args: {},
    result:
      "// Codegen template for TypeScript\npub fn generate_client(service: &ServiceDef) -> String { ... }",
    status: "success",
  },
];

// ─── Scenario Maps ────────────────────────────────────────────────────────────

export interface SessionScenario {
  detail: SessionDetail;
  captainEvents: ContentBlock[];
  mateEvents: ContentBlock[];
  taskHistory: Task[];
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
      projectName: "styx",
      branch: "feat/lsp-hover",
      autonomyMode: "human-in-the-loop",
      captain: {
        kind: "claude",
        role: "captain",
        state: "idle",
        context: { used: 8000, total: 200000 },
      },
      mate: {
        kind: "codex",
        role: "mate",
        state: "working",
        context: { used: 35000, total: 200000 },
      },
      activeTask: {
        id: "task_lsp_01",
        description: "Add hover documentation support to the Styx language server",
        status: "Working",
        createdAt: minutesAgo(60),
      },
    },
    captainEvents: CAPTAIN_EVENTS_IDLE,
    mateEvents: MATE_EVENTS_WORKING,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "mate-awaiting-permission": {
    detail: {
      id: "sess_map",
      projectName: "roam",
      branch: "feat/shared-memory-ipc",
      autonomyMode: "human-in-the-loop",
      captain: {
        kind: "claude",
        role: "captain",
        state: "working",
        context: { used: 22000, total: 200000 },
      },
      mate: {
        kind: "claude",
        role: "mate",
        state: "awaiting-permission",
        context: { used: 18000, total: 200000 },
      },
      activeTask: {
        id: "task_shm_01",
        description: "Implement lock-free shared memory ring buffer for IPC",
        status: "Working",
        createdAt: minutesAgo(30),
      },
    },
    captainEvents: CAPTAIN_EVENTS_IDLE,
    mateEvents: MATE_EVENTS_AWAITING_PERMISSION,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "agent-error": {
    detail: {
      id: "sess_err",
      projectName: "styx",
      branch: "refactor/parser-cleanup",
      autonomyMode: "human-in-the-loop",
      captain: {
        kind: "claude",
        role: "captain",
        state: "error",
        errorMessage:
          "ACP connection lost: process exited with code 1. Check that `claude-agent-acp` is installed and on PATH.",
      },
      mate: { kind: "claude", role: "mate", state: "idle", context: { used: 5000, total: 200000 } },
      activeTask: {
        id: "task_ref_01",
        description: "Refactor CST parser to reduce allocations in hot path",
        status: "Working",
        createdAt: minutesAgo(15),
      },
    },
    captainEvents: CAPTAIN_EVENTS_ERROR,
    mateEvents: MATE_EVENTS_IDLE_WAITING,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "context-exhausted": {
    detail: {
      id: "sess_ctx",
      projectName: "roam",
      branch: "feat/opentelemetry-tracing",
      autonomyMode: "human-in-the-loop",
      captain: {
        kind: "claude",
        role: "captain",
        state: "working",
        context: { used: 40000, total: 200000 },
      },
      mate: {
        kind: "claude",
        role: "mate",
        state: "context-exhausted",
        context: { used: 198000, total: 200000 },
      },
      activeTask: {
        id: "task_otel_01",
        description: "Add OpenTelemetry instrumentation to all RPC call sites",
        status: "Working",
        createdAt: minutesAgo(180),
      },
    },
    captainEvents: MOCK_CAPTAIN_EVENTS,
    mateEvents: MATE_EVENTS_CONTEXT_EXHAUSTED,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "steer-pending": {
    detail: {
      id: "sess_steer",
      projectName: "roam",
      branch: "fix/reconnect-race",
      autonomyMode: "human-in-the-loop",
      captain: {
        kind: "claude",
        role: "captain",
        state: "idle",
        context: { used: 32000, total: 200000 },
      },
      mate: {
        kind: "claude",
        role: "mate",
        state: "idle",
        context: { used: 15000, total: 200000 },
      },
      activeTask: {
        id: "task_race_01",
        description: "Fix race condition in WebSocket reconnect handler",
        status: "SteerPending",
        createdAt: minutesAgo(90),
      },
      pendingSteer: {
        id: "steer_race_01",
        captainSteer:
          "The race condition is in `reconnect()` — it modifies `self.conn` without holding the lock. Fix:\n\n1. Wrap `self.conn` in an `Arc<Mutex<Option<Connection>>>`\n2. Acquire the lock before checking and setting `conn`\n3. Release the lock before the async connect call\n4. Re-acquire to store the new connection\n\nSee `session.rs:142` for the existing locking pattern.",
      },
    },
    captainEvents: CAPTAIN_EVENTS_STEER_PENDING,
    mateEvents: MATE_EVENTS_STEER_PENDING,
    taskHistory: MOCK_TASK_HISTORY,
  },
  "no-active-task": {
    detail: {
      id: "sess_notask",
      projectName: "styx",
      branch: "feat/wasm-bindings",
      autonomyMode: "human-in-the-loop",
      captain: { kind: "claude", role: "captain", state: "idle" },
      mate: { kind: "claude", role: "mate", state: "idle" },
    },
    captainEvents: [],
    mateEvents: [],
    taskHistory: [
      ...MOCK_TASK_HISTORY,
      {
        id: "task_hist_03",
        description: "Implement WASM bindings for Styx schema validation",
        status: "Accepted",
        createdAt: minutesAgo(60),
        completedAt: minutesAgo(15),
      },
    ],
  },
  "autonomous-mode": {
    detail: {
      id: "sess_auto",
      projectName: "roam",
      branch: "feat/codegen-typescript",
      autonomyMode: "autonomous",
      captain: {
        kind: "claude",
        role: "captain",
        state: "working",
        context: { used: 12000, total: 200000 },
      },
      mate: {
        kind: "claude",
        role: "mate",
        state: "working",
        context: { used: 28000, total: 200000 },
      },
      activeTask: {
        id: "task_codegen_01",
        description: "Generate TypeScript client from roam service definitions",
        status: "Working",
        createdAt: minutesAgo(20),
      },
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
      projectName: "roam",
      branch: "feat/websocket-transport",
      captainKind: "claude",
      mateKind: "claude",
      taskDescription: "Implement WebSocket transport layer with reconnection logic and backoff",
      taskStatus: "ReviewPending",
      lastActivityAt: minutesAgo(45),
      hasIdleReminder: true,
    },
    {
      id: "sess_idle_02",
      projectName: "styx",
      branch: "feat/lsp-hover",
      captainKind: "claude",
      mateKind: "codex",
      taskDescription: "Add hover documentation support to the Styx language server",
      taskStatus: "SteerPending",
      lastActivityAt: minutesAgo(120),
      hasIdleReminder: true,
    },
    {
      id: "sess_idle_03",
      projectName: "roam",
      branch: "fix/reconnect-race",
      captainKind: "codex",
      mateKind: "claude",
      taskDescription: "Fix race condition in WebSocket reconnect handler",
      taskStatus: "Working",
      lastActivityAt: minutesAgo(5),
      hasIdleReminder: false,
    },
    {
      id: "sess_idle_04",
      projectName: "styx",
      branch: "refactor/parser-cleanup",
      captainKind: "claude",
      mateKind: "claude",
      taskDescription: "Refactor CST parser to reduce allocations in hot path",
      taskStatus: "ReviewPending",
      lastActivityAt: minutesAgo(200),
      hasIdleReminder: true,
    },
  ],
};
