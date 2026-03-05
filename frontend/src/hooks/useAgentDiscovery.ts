import { MOCK_AGENT_DISCOVERY } from "../mocks/data";
import type { AgentDiscovery } from "../types";

// r[server.agent-discovery]
export function useAgentDiscovery(): AgentDiscovery {
  return MOCK_AGENT_DISCOVERY;
}
