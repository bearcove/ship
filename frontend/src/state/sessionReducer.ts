import type { AgentSnapshot, SessionEventEnvelope, TaskStatus } from "../generated/ship";
import {
  type BlockStore,
  createBlockStore,
  appendBlock,
  patchBlock,
  clearBlocks,
} from "./blockStore";

// r[event.client.view-state]
export interface SessionViewState {
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
  captainBlocks: BlockStore;
  mateBlocks: BlockStore;
  currentTaskId: string | null;
  currentTaskStatus: TaskStatus | null;
  connected: boolean;
  replayComplete: boolean;
  lastSeq: number;
}

export function initialSessionViewState(): SessionViewState {
  return {
    captain: null,
    mate: null,
    captainBlocks: createBlockStore(),
    mateBlocks: createBlockStore(),
    currentTaskId: null,
    currentTaskStatus: null,
    connected: true,
    replayComplete: false,
    lastSeq: 0,
  };
}

export type SessionAction =
  | { type: "event"; envelope: SessionEventEnvelope }
  | { type: "replay-complete" }
  | { type: "connected" }
  | { type: "disconnected" };

// r[event.client.reducer]
// r[event.client.reducer-purity]
export function sessionReducer(state: SessionViewState, action: SessionAction): SessionViewState {
  switch (action.type) {
    case "replay-complete":
      return { ...state, replayComplete: true };

    // r[event.client.connection-lifecycle]
    case "connected":
      return { ...state, connected: true };

    // r[event.client.connection-lifecycle]
    case "disconnected":
      return {
        ...initialSessionViewState(),
        connected: false,
      };

    case "event": {
      const { envelope } = action;
      const nextState = { ...state, lastSeq: Number(envelope.seq) };
      const ev = envelope.event;

      switch (ev.tag) {
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

        case "TaskStatusChanged":
          return { ...nextState, currentTaskStatus: ev.status };

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

        case "TaskStarted":
          return {
            ...nextState,
            currentTaskId: ev.task_id,
            currentTaskStatus: { tag: "Assigned" },
            captainBlocks: clearBlocks(),
            mateBlocks: clearBlocks(),
          };
      }
    }
  }
}
