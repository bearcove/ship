import { useEffect, useState } from "react";
import { getShipClient } from "../api/client";
import type { WorktreeDiffStats } from "../generated";

export function useWorktreeDiffStats(sessionId: string): WorktreeDiffStats | null {
  const [stats, setStats] = useState<WorktreeDiffStats | null>(null);

  useEffect(() => {
    let active = true;

    async function fetch() {
      const client = await getShipClient();
      const result = await client.getWorktreeDiffStats(sessionId);
      if (active) setStats(result);
    }

    fetch();
    const interval = setInterval(fetch, 5000);
    window.addEventListener("focus", fetch);

    return () => {
      active = false;
      clearInterval(interval);
      window.removeEventListener("focus", fetch);
    };
  }, [sessionId]);

  return stats;
}
