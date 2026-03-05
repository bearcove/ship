// r[session.agent.kind]
export type AgentKind = "claude" | "codex";

export type Role = "captain" | "mate";

// r[agent-state.derived]
export type AgentState = "idle" | "working" | "awaiting-permission" | "context-exhausted" | "error";

// r[task.status.enum]
export type TaskStatus =
  | "Assigned"
  | "Working"
  | "ReviewPending"
  | "SteerPending"
  | "Accepted"
  | "Cancelled";

// r[autonomy.toggle]
export type AutonomyMode = "autonomous" | "human-in-the-loop";

export interface Project {
  name: string;
  path: string;
  valid: boolean;
  invalidReason?: string;
}

// r[session.list]
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

// r[agent-state.snapshot]
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

// r[agent-state.plan-step]
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

// r[approval.request.content]
export interface PermissionBlock {
  type: "permission";
  id: string;
  role: Role;
  toolName: string;
  description: string;
  args: Record<string, unknown>;
  resolution?: "approved" | "denied";
}

// r[event.content-block.types]
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

export type SessionScenarioKey =
  | "happy-path"
  | "captain-idle-mate-working"
  | "mate-awaiting-permission"
  | "agent-error"
  | "context-exhausted"
  | "steer-pending"
  | "no-active-task"
  | "autonomous-mode";

export type SessionListScenarioKey = "normal" | "empty" | "with-idle-reminders";
