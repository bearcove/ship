import type {
  AgentAcpInfo,
  AgentSnapshot,
  HumanReviewRequest,
  PlanStep,
  SessionDetail,
  SessionEventEnvelope,
  SessionStartupState,
  TaskStatus,
} from "../generated/ship";
import {
  type BlockStore,
  createBlockStore,
  appendBlock,
  patchBlock,
  appendBlockMut,
  patchBlockMut,
} from "./blockStore";

// r[event.client.view-state]
// r[session.agent.captain]
// r[session.agent.mate]
export interface SessionViewState {
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
  captainAcpInfo: AgentAcpInfo | null;
  mateAcpInfo: AgentAcpInfo | null;
  captainBlocks: BlockStore;
  mateBlocks: BlockStore;
  unifiedBlocks: BlockStore;
  startupState: SessionStartupState | null;
  currentTaskId: string | null;
  currentTaskTitle: string | null;
  currentTaskDescription: string | null;
  currentTaskStatus: TaskStatus | null;
  currentTaskStartedAt: string | null;
  currentTaskCompletedAt: string | null;
  currentTaskSteps: PlanStep[];
  connected: boolean;
  phase: "loading" | "replaying" | "live";
  lastSeq: number | null;
  lastEventKind: string | null;
  eventCount: number;
  replayEventCount: number;
  disconnectReason: string | null;
  connectionAttempt: number;
  pendingHumanReview: HumanReviewRequest | null;
  title: string | null;
}

export function initialSessionViewState(): SessionViewState {
  return {
    captain: null,
    mate: null,
    captainAcpInfo: null,
    mateAcpInfo: null,
    captainBlocks: createBlockStore(),
    mateBlocks: createBlockStore(),
    unifiedBlocks: createBlockStore(),
    startupState: null,
    currentTaskId: null,
    currentTaskTitle: null,
    currentTaskDescription: null,
    currentTaskStatus: null,
    currentTaskStartedAt: null,
    currentTaskCompletedAt: null,
    currentTaskSteps: [],
    connected: true,
    phase: "loading",
    lastSeq: null,
    lastEventKind: null,
    eventCount: 0,
    replayEventCount: 0,
    disconnectReason: null,
    connectionAttempt: 0,
    pendingHumanReview: null,
    title: null,
  };
}

export type SessionAction =
  | { type: "hydrate"; session: SessionDetail }
  | { type: "event"; envelope: SessionEventEnvelope }
  | { type: "replay-batch"; envelopes: SessionEventEnvelope[] }
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
        captainAcpInfo: action.session.captain_acp_info ?? null,
        mateAcpInfo: action.session.mate_acp_info ?? null,
        startupState: action.session.startup_state,
        currentTaskId: action.session.current_task?.id ?? null,
        currentTaskTitle: action.session.current_task?.title ?? null,
        currentTaskDescription: action.session.current_task?.description ?? null,
        currentTaskStatus: action.session.current_task?.status ?? null,
        currentTaskStartedAt: action.session.current_task?.assigned_at ?? null,
        currentTaskCompletedAt: action.session.current_task?.completed_at ?? null,
        currentTaskSteps:
          (action.session.current_task as unknown as { steps?: PlanStep[] })?.steps ?? [],
        title: action.session.title ?? null,
        pendingHumanReview: action.session.pending_human_review ?? null,
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

    // r[event.replay-batch]
    case "replay-batch": {
      const { envelopes } = action;
      if (envelopes.length === 0) return state;

      // Work with mutable block stores during batch, then freeze at the end
      const captainBlocks: BlockStore = {
        blocks: [...state.captainBlocks.blocks],
        index: new Map(state.captainBlocks.index),
      };
      const mateBlocks: BlockStore = {
        blocks: [...state.mateBlocks.blocks],
        index: new Map(state.mateBlocks.index),
      };
      const unifiedBlocks: BlockStore = {
        blocks: [...state.unifiedBlocks.blocks],
        index: new Map(state.unifiedBlocks.index),
      };

      let {
        captain,
        mate,
        captainAcpInfo,
        mateAcpInfo,
        startupState,
        currentTaskId,
        currentTaskTitle,
        currentTaskDescription,
        currentTaskStatus,
        currentTaskStartedAt,
        currentTaskCompletedAt,
        currentTaskSteps,
        title,
      } = state;

      for (const envelope of envelopes) {
        const ev = envelope.event;
        switch (ev.tag) {
          case "BlockAppend": {
            const store = ev.role.tag === "Captain" ? captainBlocks : mateBlocks;
            appendBlockMut(store, ev.block_id, ev.role, ev.block, envelope.timestamp);
            appendBlockMut(unifiedBlocks, ev.block_id, ev.role, ev.block, envelope.timestamp);
            break;
          }
          case "BlockPatch": {
            const store = ev.role.tag === "Captain" ? captainBlocks : mateBlocks;
            patchBlockMut(store, ev.block_id, ev.patch);
            patchBlockMut(unifiedBlocks, ev.block_id, ev.patch);
            if (ev.role.tag !== "Captain" && ev.patch.tag === "PlanReplace") {
              currentTaskSteps = ev.patch.steps;
            }
            break;
          }
          case "AgentStateChanged": {
            if (ev.role.tag === "Captain" && captain) {
              captain = { ...captain, state: ev.state };
            } else if (ev.role.tag !== "Captain" && mate) {
              mate = { ...mate, state: ev.state };
            }
            break;
          }
          case "SessionStartupChanged":
            startupState = ev.state;
            break;
          case "TaskStatusChanged":
            currentTaskStatus = ev.status;
            if (ev.status.tag === "Accepted" || ev.status.tag === "Cancelled") {
              currentTaskCompletedAt = envelope.timestamp;
            }
            break;
          case "ContextUpdated": {
            if (ev.role.tag === "Captain" && captain) {
              captain = { ...captain, context_remaining_percent: ev.remaining_percent };
            } else if (ev.role.tag !== "Captain" && mate) {
              mate = { ...mate, context_remaining_percent: ev.remaining_percent };
            }
            break;
          }
          case "TaskStarted":
            currentTaskId = ev.task_id;
            currentTaskTitle = ev.title;
            currentTaskDescription = ev.description;
            currentTaskStatus = { tag: "Assigned" };
            currentTaskStartedAt = envelope.timestamp;
            currentTaskCompletedAt = null;
            currentTaskSteps = (ev as unknown as { steps?: PlanStep[] }).steps ?? [];
            break;
          case "AgentModelChanged": {
            if (ev.role.tag === "Captain" && captain) {
              captain = {
                ...captain,
                model_id: ev.model_id,
                ...(ev.available_models.length > 0 && { available_models: ev.available_models }),
              };
            } else if (ev.role.tag !== "Captain" && mate) {
              mate = {
                ...mate,
                model_id: ev.model_id,
                ...(ev.available_models.length > 0 && { available_models: ev.available_models }),
              };
            }
            break;
          }
          // r[event.agent-effort-changed]
          case "AgentEffortChanged": {
            if (ev.role.tag === "Captain" && captain) {
              captain = {
                ...captain,
                effort_config_id: ev.effort_config_id,
                effort_value_id: ev.effort_value_id,
                ...(ev.available_effort_values.length > 0 && {
                  available_effort_values: ev.available_effort_values,
                }),
              };
            } else if (ev.role.tag !== "Captain" && mate) {
              mate = {
                ...mate,
                effort_config_id: ev.effort_config_id,
                effort_value_id: ev.effort_value_id,
                ...(ev.available_effort_values.length > 0 && {
                  available_effort_values: ev.available_effort_values,
                }),
              };
            }
            break;
          }
          // r[event.session-title-changed]
          case "SessionTitleChanged":
            title = ev.title;
            break;
          // r[acp.debug-info]
          case "AgentAcpInfoChanged":
            if (ev.role.tag === "Captain") {
              captainAcpInfo = ev.info;
            } else {
              mateAcpInfo = ev.info;
            }
            break;
        }
      }

      const lastEnvelope = envelopes[envelopes.length - 1];
      return {
        ...state,
        captain,
        mate,
        captainAcpInfo,
        mateAcpInfo,
        captainBlocks,
        mateBlocks,
        unifiedBlocks,
        startupState,
        currentTaskId,
        currentTaskTitle,
        currentTaskDescription,
        currentTaskStatus,
        currentTaskStartedAt,
        currentTaskCompletedAt,
        currentTaskSteps,
        title,
        lastSeq: Number(lastEnvelope.seq),
        lastEventKind: lastEnvelope.event.tag,
        eventCount: state.eventCount + envelopes.length,
        replayEventCount: state.replayEventCount + envelopes.length,
      };
    }

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
          const ts = envelope.timestamp;
          const unified = appendBlock(nextState.unifiedBlocks, ev.block_id, ev.role, ev.block, ts);
          if (isCaptain) {
            return {
              ...nextState,
              captainBlocks: appendBlock(
                nextState.captainBlocks,
                ev.block_id,
                ev.role,
                ev.block,
                ts,
              ),
              unifiedBlocks: unified,
            };
          }
          return {
            ...nextState,
            mateBlocks: appendBlock(nextState.mateBlocks, ev.block_id, ev.role, ev.block, ts),
            unifiedBlocks: unified,
          };
        }

        // r[event.patch]
        case "BlockPatch": {
          const isCaptain = ev.role.tag === "Captain";
          const unifiedPatched = patchBlock(nextState.unifiedBlocks, ev.block_id, ev.patch);
          if (isCaptain) {
            const patched = patchBlock(nextState.captainBlocks, ev.block_id, ev.patch);
            if (patched === null) return nextState;
            return {
              ...nextState,
              captainBlocks: patched,
              unifiedBlocks: unifiedPatched ?? nextState.unifiedBlocks,
            };
          }
          const patched = patchBlock(nextState.mateBlocks, ev.block_id, ev.patch);
          if (patched === null) return nextState;
          return {
            ...nextState,
            mateBlocks: patched,
            unifiedBlocks: unifiedPatched ?? nextState.unifiedBlocks,
            ...(ev.patch.tag === "PlanReplace" ? { currentTaskSteps: ev.patch.steps } : {}),
          };
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
          return {
            ...nextState,
            currentTaskStatus: ev.status,
            ...(ev.status.tag === "Accepted" || ev.status.tag === "Cancelled"
              ? { currentTaskCompletedAt: envelope.timestamp }
              : {}),
          };

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
            currentTaskTitle: ev.title,
            currentTaskDescription: ev.description,
            currentTaskStatus: { tag: "Assigned" },
            currentTaskStartedAt: envelope.timestamp,
            currentTaskCompletedAt: null,
            currentTaskSteps: (ev as unknown as { steps?: PlanStep[] }).steps ?? [],
          };

        case "AgentModelChanged": {
          const isCaptain = ev.role.tag === "Captain";
          if (isCaptain && nextState.captain) {
            return {
              ...nextState,
              captain: {
                ...nextState.captain,
                model_id: ev.model_id,
                ...(ev.available_models.length > 0 && {
                  available_models: ev.available_models,
                }),
              },
            };
          }
          if (!isCaptain && nextState.mate) {
            return {
              ...nextState,
              mate: {
                ...nextState.mate,
                model_id: ev.model_id,
                ...(ev.available_models.length > 0 && {
                  available_models: ev.available_models,
                }),
              },
            };
          }
          return nextState;
        }

        // r[event.agent-effort-changed]
        case "AgentEffortChanged": {
          const isCaptain = ev.role.tag === "Captain";
          if (isCaptain && nextState.captain) {
            return {
              ...nextState,
              captain: {
                ...nextState.captain,
                effort_config_id: ev.effort_config_id,
                effort_value_id: ev.effort_value_id,
                ...(ev.available_effort_values.length > 0 && {
                  available_effort_values: ev.available_effort_values,
                }),
              },
            };
          }
          if (!isCaptain && nextState.mate) {
            return {
              ...nextState,
              mate: {
                ...nextState.mate,
                effort_config_id: ev.effort_config_id,
                effort_value_id: ev.effort_value_id,
                ...(ev.available_effort_values.length > 0 && {
                  available_effort_values: ev.available_effort_values,
                }),
              },
            };
          }
          return nextState;
        }

        // r[acp.debug-info]
        case "AgentAcpInfoChanged": {
          const isCaptain = ev.role.tag === "Captain";
          if (isCaptain) {
            return { ...nextState, captainAcpInfo: ev.info };
          }
          return { ...nextState, mateAcpInfo: ev.info };
        }
        case "MateGuidanceQueued": {
          return nextState;
        }
        case "HumanReviewRequested": {
          return {
            ...nextState,
            pendingHumanReview: {
              message: ev.message,
              diff: ev.diff,
              worktree_path: ev.worktree_path,
            },
          };
        }
        case "HumanReviewCleared": {
          return { ...nextState, pendingHumanReview: null };
        }
        // r[event.session-title-changed]
        case "SessionTitleChanged":
          return { ...nextState, title: ev.title };
      }
    }
  }
}
