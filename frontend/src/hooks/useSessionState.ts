import { useSyncExternalStore } from "react";
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

// --- Module-level subscription store ---

interface SessionSubscription {
  state: SessionViewState;
  lastHydratedSession: SessionDetail | null;
  debugMessages: DebugMessage[];
  listeners: Set<() => void>;
  retryCount: number;
  cancelled: boolean;
  signalStop: ((reason: string) => void) | null;
  reconnectTimer: number | null;

  // Stable references for useSyncExternalStore
  subscribe: (onChange: () => void) => () => void;
  getSnapshot: () => SessionViewState;
}

const subscriptions = new Map<string, SessionSubscription>();

function applyAction(
  sub: SessionSubscription,
  sessionId: string,
  action: Parameters<typeof sessionReducer>[1],
) {
  const previousPhase = sub.state.phase;
  sub.state = sessionReducer(sub.state, action);

  // Debug logging for state transitions
  if (sub.state.phase === "live" && previousPhase !== "live") {
    log("info", "replay complete", {
      sessionId,
      replayEventCount: sub.state.replayEventCount,
      lastSeq: sub.state.lastSeq,
    });
  } else if (sub.state.lastSeq !== null && sub.state.lastEventKind) {
    log("debug", "applied session event", {
      sessionId,
      seq: sub.state.lastSeq,
      eventKind: sub.state.lastEventKind,
      phase: sub.state.phase,
    });
  }

  publishSessionDebug(sessionId, sub.lastHydratedSession, sub.state, sub.debugMessages);
  notifyListeners(sub);
}

function notifyListeners(sub: SessionSubscription) {
  for (const listener of sub.listeners) {
    listener();
  }
}

function startSubscription(sessionId: string, sub: SessionSubscription) {
  sub.cancelled = false;

  async function subscribe() {
    const attempt = sub.retryCount + 1;
    applyAction(sub, sessionId, { type: "connected", attempt });
    if (sub.lastHydratedSession) {
      applyAction(sub, sessionId, { type: "hydrate", session: sub.lastHydratedSession });
    }
    log("info", "starting session subscription", {
      sessionId,
      attempt,
      forceNewClient: sub.retryCount > 0,
    });

    const client = await getShipClient({ forceNew: sub.retryCount > 0 });
    if (sub.cancelled) return;

    const [tx, rx] = channel<SubscribeMessage>();

    const stopSignal = new Promise<string>((resolve) => {
      sub.signalStop = resolve;
    });

    await client.subscribeEvents(sessionId, tx);
    if (sub.cancelled) return;
    log("info", "subscription setup complete", { sessionId, attempt });

    let stopReason: string | null = null;
    let lastSeenSeq = sub.state.lastSeq;
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
      if (sub.cancelled) break;
      if (next.msg === null) {
        stopReason = "subscription channel closed";
        sub.debugMessages = [
          ...sub.debugMessages.slice(-199),
          { kind: "channel-closed", reason: stopReason, attempt },
        ];
        log("warn", "subscription channel closed", { sessionId, attempt });
        break;
      }

      if (next.msg.tag === "Event") {
        const nextSeq = Number(next.msg.value.seq);
        sub.debugMessages = [
          ...sub.debugMessages.slice(-199),
          {
            kind: "event",
            phase: sub.state.phase,
            envelope: next.msg.value,
          },
        ];
        if (!replaying && next.msg.value.event.tag === "BlockPatch") {
          const store =
            next.msg.value.event.role.tag === "Captain"
              ? sub.state.captainBlocks
              : sub.state.mateBlocks;
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
        // Gap detection only applies to live events — during replay the server
        // may coalesce BlockPatch events into their BlockAppend, producing
        // intentional sequence-number gaps that are not a sign of data loss.
        if (!replaying) {
          const gap = detectSequenceGap(lastSeenSeq, nextSeq);
          if (gap) {
            stopReason = gap;
            log("warn", "sequence gap detected", {
              sessionId,
              expectedSeq: (lastSeenSeq ?? -1) + 1,
              receivedSeq: nextSeq,
            });
            invalidateShipClient(stopReason);
            sub.signalStop?.(stopReason);
            continue;
          }
        }
        lastSeenSeq = nextSeq;
        if (replaying) {
          replayBuffer.push(next.msg.value);
        } else {
          applyAction(sub, sessionId, { type: "event", envelope: next.msg.value });
        }
      } else if (next.msg.tag === "ReplayComplete") {
        // Flush all buffered replay events in a single dispatch
        if (replayBuffer.length > 0) {
          log("info", "applying replay batch", {
            sessionId,
            eventCount: replayBuffer.length,
          });
          applyAction(sub, sessionId, { type: "replay-batch", envelopes: replayBuffer });
          replayBuffer = [];
        }
        replaying = false;
        sub.debugMessages = [
          ...sub.debugMessages.slice(-199),
          {
            kind: "replay-complete",
            phase: sub.state.phase,
            replayEventCount: sub.state.replayEventCount,
            lastSeq: sub.state.lastSeq,
          },
        ];
        log("info", "received replay complete marker", {
          sessionId,
          attempt,
          replayEventCount: sub.state.replayEventCount,
        });
        applyAction(sub, sessionId, { type: "replay-complete" });
      }
    }

    if (!sub.cancelled) {
      const reason = stopReason ?? "subscription stopped without a close reason";
      const diagnosis = diagnoseDisconnectReason(reason);
      sub.debugMessages = [
        ...sub.debugMessages.slice(-199),
        { kind: "disconnect", reason, diagnosis, attempt },
      ];
      log("warn", "session subscription stopped", {
        sessionId,
        attempt,
        reason,
        diagnosis,
      });
      applyAction(sub, sessionId, { type: "disconnected", reason });
      sub.reconnectTimer = window.setTimeout(() => {
        if (!sub.cancelled) {
          sub.retryCount++;
          startSubscription(sessionId, sub);
        }
      }, RECONNECT_DELAY_MS);
    }
  }

  subscribe().catch((error) => {
    if (!sub.cancelled) {
      const reason = `subscription setup failed: ${describeError(error)}`;
      const diagnosis = diagnoseDisconnectReason(reason);
      log("warn", "session subscription setup failed", { sessionId, reason });
      invalidateShipClient(reason);
      sub.debugMessages = [
        ...sub.debugMessages.slice(-199),
        { kind: "disconnect", reason, diagnosis, attempt: sub.retryCount + 1 },
      ];
      applyAction(sub, sessionId, { type: "disconnected", reason });
      sub.reconnectTimer = window.setTimeout(() => {
        if (!sub.cancelled) {
          sub.retryCount++;
          startSubscription(sessionId, sub);
        }
      }, RECONNECT_DELAY_MS);
    }
  });
}

function ensureSubscription(sessionId: string): SessionSubscription {
  let sub = subscriptions.get(sessionId);
  if (sub) return sub;

  sub = {
    state: initialSessionViewState(),
    lastHydratedSession: null,
    debugMessages: [],
    listeners: new Set(),
    retryCount: 0,
    cancelled: false,
    signalStop: null,
    reconnectTimer: null,
    // Stable closures that capture the subscription by reference via the Map
    subscribe: (onChange: () => void) => {
      const s = subscriptions.get(sessionId)!;
      s.listeners.add(onChange);
      return () => {
        s.listeners.delete(onChange);
      };
    },
    getSnapshot: () => subscriptions.get(sessionId)!.state,
  };

  subscriptions.set(sessionId, sub);
  startSubscription(sessionId, sub);
  return sub;
}

/** Tear down a session subscription (e.g., for tests). */
export function destroySubscription(sessionId: string) {
  const sub = subscriptions.get(sessionId);
  if (!sub) return;
  sub.cancelled = true;
  if (sub.reconnectTimer !== null) {
    window.clearTimeout(sub.reconnectTimer);
  }
  sub.signalStop?.("subscription cleanup");
  subscriptions.delete(sessionId);
}

/** Tear down all subscriptions (e.g., for tests). */
export function destroyAllSubscriptions() {
  for (const sessionId of subscriptions.keys()) {
    destroySubscription(sessionId);
  }
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
  const sub = ensureSubscription(sessionId);

  // Hydrate when session detail arrives or changes.
  // Update sub.state directly without calling notifyListeners: we are in the
  // render phase, and notifyListeners would trigger "setState during render".
  // useSyncExternalStore calls getSnapshot() immediately after this block, so
  // the updated state is reflected in the return value without an extra render.
  if (session && session !== sub.lastHydratedSession) {
    sub.lastHydratedSession = session;
    sub.state = sessionReducer(sub.state, { type: "hydrate", session });
    publishSessionDebug(sessionId, sub.lastHydratedSession, sub.state, sub.debugMessages);
  }

  return useSyncExternalStore(sub.subscribe, sub.getSnapshot);
}
