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

export type AutonomyMode = "autonomous" | "human-in-the-loop";

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

export interface AgentContext {
  used: number;
  total: number;
}

export interface AgentInfo {
  kind: AgentKind;
  role: Role;
  state: AgentState;
  context?: AgentContext;
  errorMessage?: string;
}

export interface Task {
  id: string;
  description: string;
  status: TaskStatus;
  createdAt: Date;
  completedAt?: Date;
}

export interface SessionDetail {
  id: string;
  projectName: string;
  branch: string;
  autonomyMode: AutonomyMode;
  captain: AgentInfo;
  mate: AgentInfo;
  activeTask?: Task;
  pendingSteer?: SteerReview;
}

// ─── Content Blocks ──────────────────────────────────────────────────────────

export type PlanStepStatus = "planned" | "in-progress" | "completed" | "failed";

export interface PlanStep {
  description: string;
  status: PlanStepStatus;
}

export type ToolCallStatus = "success" | "failure" | "pending";

export type ToolCallKind = "read" | "write" | "edit" | "command" | "search" | "other";

export interface ToolCallBlock {
  type: "tool-call";
  id: string;
  toolName: string;
  kind: ToolCallKind;
  filePath?: string;
  command?: string;
  query?: string;
  args: Record<string, unknown>;
  result?: string;
  status: ToolCallStatus;
  diffSummary?: string;
}

export interface TextBlock {
  type: "text";
  id: string;
  role: Role;
  content: string;
}

export interface PlanUpdateBlock {
  type: "plan-update";
  id: string;
  role: Role;
  steps: PlanStep[];
}

export interface ErrorBlock {
  type: "error";
  id: string;
  role: Role;
  message: string;
}

export interface PermissionBlock {
  type: "permission";
  id: string;
  role: Role;
  toolName: string;
  description: string;
  args: Record<string, unknown>;
  resolution?: "approved" | "denied";
}

export type ContentBlock =
  | TextBlock
  | ToolCallBlock
  | PlanUpdateBlock
  | ErrorBlock
  | PermissionBlock;

export interface SteerReview {
  id: string;
  captainSteer: string;
}

export interface AgentDiscovery {
  claude: boolean;
  codex: boolean;
}
