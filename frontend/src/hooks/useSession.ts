import { useEffect, useState } from "react";
import { shipClient } from "../api/client";
import type { SessionDetail } from "../generated/ship";

// r[proto.get-session]
// r[event.client.hydration-sequence]
export function useSession(id: string): SessionDetail | null {
  const [session, setSession] = useState<SessionDetail | null>(null);

  useEffect(() => {
    let cancelled = false;
    shipClient
      .then((client) => client.getSession(id))
      .then((detail) => {
        if (!cancelled) setSession(detail);
      });
    return () => {
      cancelled = true;
    };
  }, [id]);

  return session;
}
