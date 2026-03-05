import { useEffect, useState } from "react";
import { shipClient } from "../api/client";
import type { AgentDiscovery } from "../generated/ship";

const DEFAULT_DISCOVERY: AgentDiscovery = { claude: true, codex: true };

// r[server.agent-discovery]
export function useAgentDiscovery(): AgentDiscovery {
  const [discovery, setDiscovery] = useState(DEFAULT_DISCOVERY);

  useEffect(() => {
    let active = true;

    shipClient
      .then((client) => client.agentDiscovery())
      .then((result) => {
        if (active) {
          setDiscovery(result);
        }
      })
      .catch(() => {});

    return () => {
      active = false;
    };
  }, []);

  return discovery;
}
