import { useEffect, useState } from "react";
import { shipClient } from "../api/client";
import type { SessionSummary } from "../generated/ship";

// r[proto.list-sessions]
export function useSessionList(projectFilter?: string): SessionSummary[] {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);

  useEffect(() => {
    let active = true;

    async function fetchSessions() {
      const client = await shipClient;
      const list = await client.listSessions();
      if (active) setSessions(list);
    }

    fetchSessions();

    window.addEventListener("focus", fetchSessions);
    return () => {
      active = false;
      window.removeEventListener("focus", fetchSessions);
    };
  }, []);

  if (!projectFilter) return sessions;
  return sessions.filter((s) => s.project === projectFilter);
}
