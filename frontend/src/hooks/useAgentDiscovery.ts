import { useEffect, useState } from "react";
import { getShipClient } from "../api/client";
import type { AgentDiscovery } from "../generated/ship";

// r[server.agent-discovery]
export function useAgentDiscovery(): AgentDiscovery {
  const [discovery, setDiscovery] = useState<AgentDiscovery>({ claude: false, codex: false });

  useEffect(() => {
    let active = true;

    async function fetchDiscovery() {
      const client = await getShipClient();
      const result = await client.agentDiscovery();
      if (active) {
        setDiscovery(result);
      }
    }

    fetchDiscovery();

    return () => {
      active = false;
    };
  }, []);

  return discovery;
}
