export type ToolCallKind = "read" | "write" | "edit" | "command" | "search" | "other";

export interface AgentDiscovery {
  claude: boolean;
  codex: boolean;
}
