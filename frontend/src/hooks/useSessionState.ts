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
    // signalStop is set once the stopSignal Promise is created inside subscribe().
    // Calling it resolves the stopSignal, immediately unblocking the receive loop.
    let signalStop: (() => void) | null = null;

    async function subscribe() {
      const client = await shipClient;
      if (cancelled) return;

      dispatch({ type: "connected" });

      const [tx, rx] = channel<SubscribeMessage>();

      // Resolves to null when the receive loop should stop (cleanup or RPC end).
      const stopSignal = new Promise<null>((resolve) => {
        signalStop = () => resolve(null);
      });

      // Start the subscription RPC eagerly (binds channel, sends request).
      // When it ends (success or error), unblock the receive loop via stopSignal.
      const subscribeCall = client.subscribeEvents(sessionId, tx);
      void subscribeCall.then(
        () => signalStop?.(),
        () => signalStop?.(),
      );

      while (true) {
        // Race between the next channel message and a stop signal.
        // This ensures cleanup (cancelled = true + signalStop()) unblocks immediately
        // instead of waiting for the next message or channel close.
        const msg = await Promise.race([rx.recv(), stopSignal]);
        if (msg === null || cancelled) break;
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
      signalStop?.(); // Unblock the receive loop immediately on cleanup
    };
  }, [sessionId, retryCount]);

  return state;
}
