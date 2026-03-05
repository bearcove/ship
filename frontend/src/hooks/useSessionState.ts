import { useEffect, useReducer } from "react";
import { channel } from "@bearcove/roam-core";
import type { SubscribeMessage } from "../generated/ship";
import { shipClient } from "../api/client";
import {
  type SessionViewState,
  initialSessionViewState,
  sessionReducer,
} from "../state/sessionReducer";

// r[event.client.hydration-sequence]
// r[event.subscribe]
export function useSessionState(sessionId: string): SessionViewState {
  const [state, dispatch] = useReducer(sessionReducer, undefined, initialSessionViewState);

  useEffect(() => {
    let cancelled = false;

    async function subscribe() {
      const client = await shipClient;
      if (cancelled) return;

      dispatch({ type: "connected" });

      const [tx, rx] = channel<SubscribeMessage>();
      // Starts the RPC call eagerly, binding the channel
      client.subscribeEvents(sessionId, tx);

      for await (const msg of rx) {
        if (cancelled) break;
        if (msg.tag === "Event") {
          dispatch({ type: "event", envelope: msg.value });
        } else if (msg.tag === "ReplayComplete") {
          dispatch({ type: "replay-complete" });
        }
      }

      if (!cancelled) {
        dispatch({ type: "disconnected" });
      }
    }

    subscribe().catch(() => {
      if (!cancelled) dispatch({ type: "disconnected" });
    });

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  return state;
}
