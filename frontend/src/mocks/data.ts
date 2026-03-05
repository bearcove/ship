import type { AgentDiscovery, Project, SessionSummary } from "../types";

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
