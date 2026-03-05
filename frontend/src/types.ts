export type ToolCallKind = "read" | "write" | "edit" | "command" | "search" | "other";

export interface SteerReview {
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
