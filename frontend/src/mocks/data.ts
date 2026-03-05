import type {
  AgentDiscovery,
  ContentBlock,
  Project,
  SessionDetail,
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
