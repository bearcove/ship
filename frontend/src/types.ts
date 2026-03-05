export type AgentKind = "claude" | "codex";

export type Role = "captain" | "mate";

export type AgentState = "idle" | "working" | "awaiting-permission" | "context-exhausted" | "error";

export type TaskStatus =
  | "Assigned"
  | "Working"
  | "ReviewPending"
  | "SteerPending"
  | "Accepted"
  | "Cancelled";

export interface Project {
  name: string;
  path: string;
  valid: boolean;
  invalidReason?: string;
}

export interface SessionSummary {
  id: string;
  projectName: string;
  branch: string;
  captainKind: AgentKind;
  mateKind: AgentKind;
  taskDescription: string;
  taskStatus: TaskStatus;
  lastActivityAt: Date;
  hasIdleReminder: boolean;
}

export interface AgentDiscovery {
  claude: boolean;
  codex: boolean;
}
