import { useEffect, useState } from "react";
import { getShipClient, invalidateShipClient, onClientReady } from "../api/client";
import type { SessionSummary } from "../generated/ship";

type SessionListListener = (sessions: SessionSummary[]) => void;

let cachedSessions: SessionSummary[] = [];
let hasFetchedSessionList = false;
let pendingRefresh: Promise<SessionSummary[]> | null = null;
const sessionListListeners = new Set<SessionListListener>();

function publishSessionList(list: SessionSummary[]) {
  cachedSessions = list;
  hasFetchedSessionList = true;

  for (const listener of sessionListListeners) {
    listener(list);
  }
}

export async function refreshSessionList(): Promise<SessionSummary[]> {
  if (pendingRefresh) {
    return pendingRefresh;
  }

  pendingRefresh = (async () => {
    const client = await getShipClient();
    const list = await client.listSessions();
    publishSessionList(list);
    return list;
  })()
    .catch((e) => {
      invalidateShipClient(`listSessions failed: ${e}`);
      throw e;
    })
    .finally(() => {
      pendingRefresh = null;
    });

  return pendingRefresh;
}

// r[proto.list-sessions]
export function useSessionList(projectFilter?: string): SessionSummary[] {
  const [sessions, setSessions] = useState<SessionSummary[]>(cachedSessions);

  useEffect(() => {
    let active = true;

    function handleSessionList(list: SessionSummary[]) {
      if (active) {
        setSessions(list);
      }
    }

    sessionListListeners.add(handleSessionList);
    handleSessionList(cachedSessions);

    if (!hasFetchedSessionList) {
      void refreshSessionList();
    }

    function handleFocus() {
      void refreshSessionList();
    }

    const unsubReady = onClientReady(() => void refreshSessionList());
    window.addEventListener("focus", handleFocus);
    return () => {
      active = false;
      unsubReady();
      sessionListListeners.delete(handleSessionList);
      window.removeEventListener("focus", handleFocus);
    };
  }, []);

  if (!projectFilter) return sessions;
  return sessions.filter((s) => s.project === projectFilter);
}
