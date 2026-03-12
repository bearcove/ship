import { useEffect, useState } from "react";
import { getShipClient, invalidateShipClient } from "../api/client";
import type { SessionSummary } from "../generated/ship";
import { onSessionListChanged } from "./useGlobalEvents";

export async function refreshSessionList(): Promise<SessionSummary[]> {
  try {
    const client = await getShipClient();
    return await client.listSessions();
  } catch (e) {
    invalidateShipClient(`listSessions failed: ${e}`);
    throw e;
  }
}

// r[proto.list-sessions]
export function useSessionList(projectFilter?: string): SessionSummary[] {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);

  useEffect(() => {
    return onSessionListChanged(setSessions);
  }, []);

  if (!projectFilter) return sessions;
  return sessions.filter((s) => s.project === projectFilter);
}
