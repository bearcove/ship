import { useEffect, useState } from "react";
import { getShipClient } from "../api/client";
import type { SessionDetail } from "../generated/ship";

// r[proto.get-session]
// r[event.client.hydration-sequence]
export function useSession(id: string): { session: SessionDetail | null; error: string | null } {
  const [session, setSession] = useState<SessionDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSession(null);
    setError(null);
    getShipClient()
      .then((client) => client.getSession(id))
      .then((detail) => {
        if (!cancelled) setSession(detail);
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : "session not found");
      });
    return () => {
      cancelled = true;
    };
  }, [id]);

  return { session, error };
}
