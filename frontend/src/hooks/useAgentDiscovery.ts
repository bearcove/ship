import type { AgentDiscovery } from "../types";

// r[server.agent-discovery]
export function useAgentDiscovery(): AgentDiscovery {
  return { claude: true, codex: true };
}
