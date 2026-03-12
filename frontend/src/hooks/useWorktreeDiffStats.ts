import { useEffect, useState } from "react";
import { getShipClient } from "../api/client";
import type { WorktreeDiffStats } from "../generated/ship";

export function useWorktreeDiffStats(sessionId: string): WorktreeDiffStats | null {
  const [stats, setStats] = useState<WorktreeDiffStats | null>(null);

  useEffect(() => {
    let cancelled = false;
    setStats(null);
    getShipClient()
      .then((c) => c.getWorktreeDiffStats(sessionId))
      .then((result) => {
        if (!cancelled) setStats(result);
      })
      .catch(() => {
        // stay null on error
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  return stats;
}
