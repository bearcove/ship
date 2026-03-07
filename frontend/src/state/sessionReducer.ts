import type {
  AgentSnapshot,
  SessionDetail,
  SessionEventEnvelope,
  SessionStartupState,
  TaskStatus,
} from "../generated/ship";
import { type BlockStore, createBlockStore, appendBlock, patchBlock } from "./blockStore";

// r[event.client.view-state]
// r[session.agent.captain]
// r[session.agent.mate]
export interface SessionViewState {
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
  captainBlocks: BlockStore;
  mateBlocks: BlockStore;
  startupState: SessionStartupState | null;
  currentTaskId: string | null;
  currentTaskDescription: string | null;
  currentTaskStatus: TaskStatus | null;
  connected: boolean;
  phase: "loading" | "replaying" | "live";
  lastSeq: number | null;
  lastEventKind: string | null;
  eventCount: number;
  replayEventCount: number;
  disconnectReason: string | null;
  connectionAttempt: number;
}

export function initialSessionViewState(): SessionViewState {
  return {
    captain: null,
    mate: null,
    captainBlocks: createBlockStore(),
    mateBlocks: createBlockStore(),
    startupState: null,
    currentTaskId: null,
    currentTaskDescription: null,
    currentTaskStatus: null,
    connected: true,
    phase: "loading",
    lastSeq: null,
    lastEventKind: null,
    eventCount: 0,
    replayEventCount: 0,
    disconnectReason: null,
    connectionAttempt: 0,
  };
}

export type SessionAction =
  | { type: "hydrate"; session: SessionDetail }
  | { type: "event"; envelope: SessionEventEnvelope }
  | { type: "replay-complete" }
  | { type: "connected"; attempt: number }
  | { type: "disconnected"; reason: string };

// r[event.client.reducer]
// r[event.client.reducer-purity]
export function sessionReducer(state: SessionViewState, action: SessionAction): SessionViewState {
  switch (action.type) {
    case "hydrate":
      return {
        ...state,
        captain: action.session.captain,
        mate: action.session.mate,
        startupState: action.session.startup_state,
        currentTaskId: action.session.current_task?.id ?? null,
        currentTaskDescription: action.session.current_task?.description ?? null,
        currentTaskStatus: action.session.current_task?.status ?? null,
      };

    // r[event.replay-complete]
    case "replay-complete":
      return { ...state, phase: "live" };

    // r[event.client.connection-lifecycle]
    case "connected":
      return {
        ...initialSessionViewState(),
        connected: true,
        phase: "replaying",
        connectionAttempt: action.attempt,
      };

    // r[event.client.connection-lifecycle]
    case "disconnected":
      return {
        ...initialSessionViewState(),
        connected: false,
        lastSeq: state.lastSeq,
        lastEventKind: state.lastEventKind,
        eventCount: state.eventCount,
        replayEventCount: state.replayEventCount,
        disconnectReason: action.reason,
        connectionAttempt: state.connectionAttempt,
      };

    case "event": {
      const { envelope } = action;
      const nextState = {
        ...state,
        lastSeq: Number(envelope.seq),
        lastEventKind: envelope.event.tag,
        eventCount: state.eventCount + 1,
        replayEventCount:
          state.phase === "replaying" ? state.replayEventCount + 1 : state.replayEventCount,
      };
      const ev = envelope.event;

      // r[event.envelope]
      // r[event.ordering]
      switch (ev.tag) {
        // r[event.append]
        case "BlockAppend": {
          const isCaptain = ev.role.tag === "Captain";
          if (isCaptain) {
            return {
              ...nextState,
              captainBlocks: appendBlock(nextState.captainBlocks, ev.block_id, ev.role, ev.block),
            };
          }
          return {
            ...nextState,
            mateBlocks: appendBlock(nextState.mateBlocks, ev.block_id, ev.role, ev.block),
          };
        }

        // r[event.patch]
        case "BlockPatch": {
          const isCaptain = ev.role.tag === "Captain";
          if (isCaptain) {
            const patched = patchBlock(nextState.captainBlocks, ev.block_id, ev.patch);
            if (patched === null) return nextState;
            return { ...nextState, captainBlocks: patched };
          }
          const patched = patchBlock(nextState.mateBlocks, ev.block_id, ev.patch);
          if (patched === null) return nextState;
          return { ...nextState, mateBlocks: patched };
        }

        // r[event.agent-state-changed]
        case "AgentStateChanged": {
          const isCaptain = ev.role.tag === "Captain";
          if (isCaptain && nextState.captain) {
            return { ...nextState, captain: { ...nextState.captain, state: ev.state } };
          }
          if (!isCaptain && nextState.mate) {
            return { ...nextState, mate: { ...nextState.mate, state: ev.state } };
          }
          return nextState;
        }

        case "SessionStartupChanged":
          return { ...nextState, startupState: ev.state };

        // r[event.task-status-changed]
        case "TaskStatusChanged":
          return { ...nextState, currentTaskStatus: ev.status };

        // r[event.context-updated]
        case "ContextUpdated": {
          const isCaptain = ev.role.tag === "Captain";
          if (isCaptain && nextState.captain) {
            return {
              ...nextState,
              captain: { ...nextState.captain, context_remaining_percent: ev.remaining_percent },
            };
          }
          if (!isCaptain && nextState.mate) {
            return {
              ...nextState,
              mate: { ...nextState.mate, context_remaining_percent: ev.remaining_percent },
            };
          }
          return nextState;
        }

        // r[event.task-started]
        case "TaskStarted":
          return {
            ...nextState,
            currentTaskId: ev.task_id,
            currentTaskDescription: ev.description,
            currentTaskStatus: { tag: "Assigned" },
          };

        case "AgentModelChanged": {
          const isCaptain = ev.role.tag === "Captain";
          if (isCaptain && nextState.captain) {
            return { ...nextState, captain: { ...nextState.captain, model_id: ev.model_id } };
          }
          if (!isCaptain && nextState.mate) {
            return { ...nextState, mate: { ...nextState.mate, model_id: ev.model_id } };
          }
          return nextState;
        }
      }
    }
  }
}
