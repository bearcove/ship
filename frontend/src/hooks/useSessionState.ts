import { useEffect, useReducer, useRef, useState } from "react";
import { channel } from "@bearcove/roam-core";
import type { SessionDetail, SessionEventEnvelope, SubscribeMessage } from "../generated/ship";
import { getShipClient, invalidateShipClient } from "../api/client";
import {
  type SessionViewState,
  initialSessionViewState,
  sessionReducer,
} from "../state/sessionReducer";
import type { BlockStore } from "../state/blockStore";

const RECONNECT_DELAY_MS = 3000;

type DebugMessage =
  | {
      kind: "event";
      phase: SessionViewState["phase"];
      envelope: SessionEventEnvelope;
    }
  | {
      kind: "replay-complete";
      phase: SessionViewState["phase"];
      replayEventCount: number;
      lastSeq: number | null;
    }
  | {
      kind: "channel-closed";
      reason: string;
      attempt: number;
    }
  | {
      kind: "disconnect";
      reason: string;
      diagnosis: DisconnectDiagnosis;
      attempt: number;
    };

type SessionDebugSnapshot = {
  session: SessionDetail | null;
  state: SessionViewState;
  messages: DebugMessage[];
};

type ShipDebugWindow = Window & {
  __shipDebug?: {
    sessions: Record<string, SessionDebugSnapshot>;
    clearSession(sessionId: string): void;
    clearAll(): void;
  };
};

function debugWindow(): ShipDebugWindow {
  return window as ShipDebugWindow;
}

function ensureShipDebug() {
  const debug = debugWindow();
  if (!debug.__shipDebug) {
    debug.__shipDebug = {
      sessions: {},
      clearSession(sessionId: string) {
        delete this.sessions[sessionId];
      },
      clearAll() {
        this.sessions = {};
      },
    };
  }
  return debug.__shipDebug;
}

function publishSessionDebug(
  sessionId: string,
  session: SessionDetail | null,
  state: SessionViewState,
  messages: DebugMessage[],
) {
  ensureShipDebug().sessions[sessionId] = {
    session,
    state,
    messages,
  };
}

export function detectSequenceGap(lastSeenSeq: number | null, nextSeq: number): string | null {
  if (lastSeenSeq === null || nextSeq === lastSeenSeq + 1) {
    return null;
  }
  return `sequence gap detected: expected ${lastSeenSeq + 1}, received ${nextSeq}`;
}

export type DisconnectDiagnosis = "real-disconnect" | "expected-reconnect" | "cleanup" | "other";

export function diagnoseDisconnectReason(reason: string): DisconnectDiagnosis {
  if (reason === "subscription channel closed") {
    return "real-disconnect";
  }
  if (reason === "subscription cleanup") {
    return "cleanup";
  }
  if (
    reason.startsWith("subscription setup failed:") ||
    reason.startsWith("sequence gap detected:")
  ) {
    return "expected-reconnect";
  }
  return "other";
}

function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return String(error);
}

function log(level: "debug" | "info" | "warn", message: string, details: Record<string, unknown>) {
  const method = level === "warn" ? console.warn : level === "info" ? console.info : console.debug;
  method(`[ship/session] ${message}`, details);
}

function expectedBlockTagForPatch(
  tag: "TextAppend" | "ToolCallUpdate" | "PlanReplace" | "PermissionResolve",
) {
  switch (tag) {
    case "TextAppend":
      return "Text";
    case "ToolCallUpdate":
      return "ToolCall";
    case "PlanReplace":
      return "PlanUpdate";
    case "PermissionResolve":
      return "Permission";
  }
}

function inspectPatchApplicability(store: BlockStore, blockId: string, patchTag: string) {
  const pos = store.index.get(blockId);
  if (pos === undefined) {
    return {
      ok: false,
      reason: "unknown-block-id" as const,
      knownBlockIds: store.blocks.map((entry) => entry.blockId),
    };
  }

  const entry = store.blocks[pos];
  const expectedTag = expectedBlockTagForPatch(
    patchTag as "TextAppend" | "ToolCallUpdate" | "PlanReplace" | "PermissionResolve",
  );
  if (entry.block.tag !== expectedTag) {
    return {
      ok: false,
      reason: "block-tag-mismatch" as const,
      actualTag: entry.block.tag,
      expectedTag,
      knownBlockIds: store.blocks.map((candidate) => candidate.blockId),
    };
  }

  return { ok: true as const };
}

// r[proto.hydration-flow]
// r[event.client.hydration-sequence]
// r[event.client.connection-lifecycle]
// r[event.subscribe]
// r[session.persistent]
export function useSessionState(
  sessionId: string,
  session: SessionDetail | null,
): SessionViewState {
  const [state, dispatch] = useReducer(sessionReducer, undefined, initialSessionViewState);
  const [retryCount, setRetryCount] = useState(0);
  const stateRef = useRef(state);
  const sessionRef = useRef(session);
  const debugMessagesRef = useRef<DebugMessage[]>([]);
  const previousPhaseRef = useRef(state.phase);

  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  useEffect(() => {
    const previousPhase = previousPhaseRef.current;
    previousPhaseRef.current = state.phase;

    if (state.phase === "live" && previousPhase !== "live") {
      log("info", "replay complete", {
        sessionId,
        replayEventCount: state.replayEventCount,
        lastSeq: state.lastSeq,
      });
      return;
    }

    if (state.lastSeq !== null && state.lastEventKind) {
      log("debug", "applied session event", {
        sessionId,
        seq: state.lastSeq,
        eventKind: state.lastEventKind,
        phase: state.phase,
      });
    }
  }, [sessionId, state.lastEventKind, state.lastSeq, state.phase, state.replayEventCount]);

  useEffect(() => {
    sessionRef.current = session;
    if (session) {
      dispatch({ type: "hydrate", session });
    }
  }, [session]);

  useEffect(() => {
    publishSessionDebug(sessionId, sessionRef.current, state, debugMessagesRef.current);
  }, [sessionId, state]);

  useEffect(() => {
    if (state.phase === "replaying") {
      log("info", "replay in progress", {
        sessionId,
        replayEventCount: state.replayEventCount,
        attempt: state.connectionAttempt,
      });
    }
  }, [sessionId, state.connectionAttempt, state.phase, state.replayEventCount]);

  useEffect(() => {
    let cancelled = false;
    let reconnectTimer: number | null = null;
    let signalStop: ((reason: string) => void) | null = null;

    async function subscribe() {
      const attempt = retryCount + 1;
      dispatch({ type: "connected", attempt });
      if (sessionRef.current) {
        dispatch({ type: "hydrate", session: sessionRef.current });
      }
      log("info", "starting session subscription", {
        sessionId,
        attempt,
        forceNewClient: retryCount > 0,
      });

      const client = await getShipClient({ forceNew: retryCount > 0 });
      if (cancelled) return;

      const [tx, rx] = channel<SubscribeMessage>();

      const stopSignal = new Promise<string>((resolve) => {
        signalStop = resolve;
      });

      await client.subscribeEvents(sessionId, tx);
      if (cancelled) return;
      log("info", "subscription setup complete", { sessionId, attempt });

      let stopReason: string | null = null;
      let lastSeenSeq = stateRef.current.lastSeq;
      let replayBuffer: SessionEventEnvelope[] = [];
      let replaying = true;

      while (true) {
        const next = await Promise.race([
          rx.recv().then((msg) => ({ tag: "message" as const, msg })),
          stopSignal.then((reason) => ({ tag: "stop" as const, reason })),
        ]);
        if (next.tag === "stop") {
          stopReason = next.reason;
          break;
        }
        if (cancelled) break;
        if (next.msg === null) {
          stopReason = "subscription channel closed";
          debugMessagesRef.current = [
            ...debugMessagesRef.current.slice(-199),
            { kind: "channel-closed", reason: stopReason, attempt },
          ];
          log("warn", "subscription channel closed", { sessionId, attempt });
          invalidateShipClient(stopReason);
          break;
        }

        if (next.msg.tag === "Event") {
          const nextSeq = Number(next.msg.value.seq);
          debugMessagesRef.current = [
            ...debugMessagesRef.current.slice(-199),
            {
              kind: "event",
              phase: stateRef.current.phase,
              envelope: next.msg.value,
            },
          ];
          if (!replaying && next.msg.value.event.tag === "BlockPatch") {
            const store =
              next.msg.value.event.role.tag === "Captain"
                ? stateRef.current.captainBlocks
                : stateRef.current.mateBlocks;
            const patchCheck = inspectPatchApplicability(
              store,
              next.msg.value.event.block_id,
              next.msg.value.event.patch.tag,
            );
            if (!patchCheck.ok) {
              log("warn", "received unappliable block patch", {
                sessionId,
                seq: nextSeq,
                role: next.msg.value.event.role.tag,
                blockId: next.msg.value.event.block_id,
                patchTag: next.msg.value.event.patch.tag,
                reason: patchCheck.reason,
                actualTag: "actualTag" in patchCheck ? patchCheck.actualTag : null,
                expectedTag: "expectedTag" in patchCheck ? patchCheck.expectedTag : null,
                knownBlockIds: patchCheck.knownBlockIds.slice(-8),
              });
            }
          }
          log("debug", "received session event", {
            sessionId,
            seq: nextSeq,
            eventKind: next.msg.value.event.tag,
            phase: replaying ? "replaying" : "live",
          });
          const gap = detectSequenceGap(lastSeenSeq, nextSeq);
          if (gap) {
            stopReason = gap;
            log("warn", "sequence gap detected", {
              sessionId,
              expectedSeq: (lastSeenSeq ?? -1) + 1,
              receivedSeq: nextSeq,
            });
            invalidateShipClient(stopReason);
            signalStop?.(stopReason);
            continue;
          }
          lastSeenSeq = nextSeq;
          if (replaying) {
            replayBuffer.push(next.msg.value);
          } else {
            dispatch({ type: "event", envelope: next.msg.value });
          }
        } else if (next.msg.tag === "ReplayComplete") {
          // Flush all buffered replay events in a single dispatch
          if (replayBuffer.length > 0) {
            log("info", "applying replay batch", {
              sessionId,
              eventCount: replayBuffer.length,
            });
            dispatch({ type: "replay-batch", envelopes: replayBuffer });
            replayBuffer = [];
          }
          replaying = false;
          debugMessagesRef.current = [
            ...debugMessagesRef.current.slice(-199),
            {
              kind: "replay-complete",
              phase: stateRef.current.phase,
              replayEventCount: stateRef.current.replayEventCount,
              lastSeq: stateRef.current.lastSeq,
            },
          ];
          log("info", "received replay complete marker", {
            sessionId,
            attempt,
            replayEventCount: stateRef.current.replayEventCount,
          });
          dispatch({ type: "replay-complete" });
        }
      }

      if (!cancelled) {
        const reason = stopReason ?? "subscription stopped without a close reason";
        const diagnosis = diagnoseDisconnectReason(reason);
        debugMessagesRef.current = [
          ...debugMessagesRef.current.slice(-199),
          { kind: "disconnect", reason, diagnosis, attempt },
        ];
        log("warn", "session subscription stopped", {
          sessionId,
          attempt,
          reason,
          diagnosis,
        });
        dispatch({ type: "disconnected", reason });
        reconnectTimer = window.setTimeout(() => {
          if (!cancelled) setRetryCount((c) => c + 1);
        }, RECONNECT_DELAY_MS);
      }
    }

    subscribe().catch((error) => {
      if (!cancelled) {
        const reason = `subscription setup failed: ${describeError(error)}`;
        const diagnosis = diagnoseDisconnectReason(reason);
        log("warn", "session subscription setup failed", { sessionId, reason });
        invalidateShipClient(reason);
        debugMessagesRef.current = [
          ...debugMessagesRef.current.slice(-199),
          { kind: "disconnect", reason, diagnosis, attempt: retryCount + 1 },
        ];
        dispatch({ type: "disconnected", reason });
        reconnectTimer = window.setTimeout(() => {
          if (!cancelled) setRetryCount((c) => c + 1);
        }, RECONNECT_DELAY_MS);
      }
    });

    return () => {
      cancelled = true;
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
      }
      signalStop?.("subscription cleanup");
    };
  }, [retryCount, sessionId]);

  return state;
}
