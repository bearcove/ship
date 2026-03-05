import { useEffect, useReducer, useState } from "react";
import { channel } from "@bearcove/roam-core";
import type { SubscribeMessage } from "../generated/ship";
import { shipClient } from "../api/client";
import {
  type SessionViewState,
  initialSessionViewState,
  sessionReducer,
} from "../state/sessionReducer";

const RECONNECT_DELAY_MS = 3000;

// r[proto.hydration-flow]
// r[event.client.hydration-sequence]
// r[event.client.connection-lifecycle]
// r[event.subscribe]
export function useSessionState(sessionId: string): SessionViewState {
  const [state, dispatch] = useReducer(sessionReducer, undefined, initialSessionViewState);
  const [retryCount, setRetryCount] = useState(0);

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
        setTimeout(() => {
          if (!cancelled) setRetryCount((c) => c + 1);
        }, RECONNECT_DELAY_MS);
      }
    }

    subscribe().catch(() => {
      if (!cancelled) {
        dispatch({ type: "disconnected" });
        setTimeout(() => {
          if (!cancelled) setRetryCount((c) => c + 1);
        }, RECONNECT_DELAY_MS);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [sessionId, retryCount]);

  return state;
}
