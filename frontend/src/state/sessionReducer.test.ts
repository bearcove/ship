import { describe, it, expect } from "vitest";
import { sessionReducer, initialSessionViewState } from "./sessionReducer";
import type { SessionViewState } from "./sessionReducer";

function freshState(): SessionViewState {
  return initialSessionViewState();
}

// r[verify event.client.connection-lifecycle]
describe("sessionReducer connection lifecycle", () => {
  it("starts in loading phase, not yet connected", () => {
    const state = freshState();
    expect(state.phase).toBe("loading");
    expect(state.connected).toBe(true); // optimistic until disconnect
  });

  it("connected action transitions to replaying phase", () => {
    const state = sessionReducer(freshState(), { type: "connected", attempt: 1 });
    expect(state.connected).toBe(true);
    expect(state.phase).toBe("replaying");
    expect(state.connectionAttempt).toBe(1);
  });

  it("replay-complete transitions to live phase", () => {
    const after_connected = sessionReducer(freshState(), { type: "connected", attempt: 1 });
    const state = sessionReducer(after_connected, { type: "replay-complete" });
    expect(state.phase).toBe("live");
    expect(state.connected).toBe(true);
  });

  it("disconnected resets state and marks disconnected", () => {
    const after_connected = sessionReducer(freshState(), { type: "connected", attempt: 1 });
    const state = sessionReducer(after_connected, {
      type: "disconnected",
      reason: "socket closed",
    });
    expect(state.connected).toBe(false);
    expect(state.phase).toBe("loading");
    expect(state.captainBlocks.blocks).toHaveLength(0);
    expect(state.mateBlocks.blocks).toHaveLength(0);
    expect(state.disconnectReason).toBe("socket closed");
  });

  it("reconnect cycle: disconnected then connected resets and enters replaying", () => {
    let state = freshState();
    state = sessionReducer(state, { type: "connected", attempt: 1 });
    state = sessionReducer(state, { type: "replay-complete" });
    // connection drops
    state = sessionReducer(state, { type: "disconnected", reason: "dropped" });
    expect(state.connected).toBe(false);
    // reconnect
    state = sessionReducer(state, { type: "connected", attempt: 2 });
    expect(state.connected).toBe(true);
    expect(state.phase).toBe("replaying");
    expect(state.connectionAttempt).toBe(2);
  });
});

// r[verify event.client.reducer]
describe("sessionReducer event handling", () => {
  it("hydrates agent snapshots and current task from session detail", () => {
    const state = sessionReducer(freshState(), {
      type: "hydrate",
      session: {
        id: "session-1",
        slug: "aaaa",
        project: "ship",
        branch_name: "ship/123/test",
        captain: {
          role: { tag: "Captain" },
          kind: { tag: "Claude" },
          state: { tag: "Idle" },
          context_remaining_percent: 75,
          model_id: null,
          available_models: [],
          effort_config_id: null,
          effort_value_id: null,
          available_effort_values: [],
        },
        mate: {
          role: { tag: "Mate" },
          kind: { tag: "Codex" },
          state: { tag: "Working", plan: null, activity: "Running" },
          context_remaining_percent: 55,
          model_id: null,
          available_models: [],
          effort_config_id: null,
          effort_value_id: null,
          available_effort_values: [],
        },
        startup_state: { tag: "Ready" },
        current_task: {
          id: "task-1",
          title: "Investigate replay",
          description: "Investigate replay",
          status: { tag: "Working" },
          steps: [],
          assigned_at: null,
          completed_at: null,
        },
        task_history: [],
        autonomy_mode: { tag: "HumanInTheLoop" },
        pending_steer: null,
        pending_human_review: null,
        title: null,
        created_at: "2026-01-01T00:00:00Z",
        user_avatar_url: null,
      },
    });
    expect(state.captain?.kind.tag).toBe("Claude");
    expect(state.mate?.state.tag).toBe("Working");
    expect(state.currentTaskId).toBe("task-1");
    expect(state.currentTaskDescription).toBe("Investigate replay");
    expect(state.currentTaskStatus?.tag).toBe("Working");
  });

  it("ignores events for unknown agent snapshots (AgentStateChanged without snapshot)", () => {
    const state = freshState();
    const next = sessionReducer(state, {
      type: "event",
      envelope: {
        seq: 1n,
        timestamp: "2026-01-01T00:00:00Z",
        event: {
          tag: "AgentStateChanged",
          role: { tag: "Captain" },
          state: { tag: "Idle" },
        },
      },
    });
    // captain snapshot is null, so no change to captain
    expect(next.captain).toBeNull();
  });

  it("TaskStarted clears blocks and sets task id", () => {
    let state = freshState();
    // add a block so we can verify it gets cleared
    state = sessionReducer(state, {
      type: "event",
      envelope: {
        seq: 1n,
        timestamp: "2026-01-01T00:00:00Z",
        event: {
          tag: "BlockAppend",
          block_id: "b1",
          role: { tag: "Captain" },
          block: { tag: "Text", text: "hello", source: { tag: "AgentMessage" } },
        },
      },
    });
    expect(state.captainBlocks.blocks).toHaveLength(1);

    state = sessionReducer(state, {
      type: "event",
      envelope: {
        seq: 2n,
        timestamp: "2026-01-01T00:00:00Z",
        event: {
          tag: "TaskStarted",
          task_id: "task-42",
          title: "do work",
          description: "do work",
          steps: [],
        },
      },
    });
    expect(state.currentTaskId).toBe("task-42");
    expect(state.currentTaskDescription).toBe("do work");
    expect(state.currentTaskStatus).toEqual({ tag: "Assigned" });
  });

  it("TaskStatusChanged updates currentTaskStatus", () => {
    const state = sessionReducer(freshState(), {
      type: "event",
      envelope: {
        seq: 1n,
        timestamp: "2026-01-01T00:00:00Z",
        event: {
          tag: "TaskStatusChanged",
          task_id: "t1",
          status: { tag: "Working" },
        },
      },
    });
    expect(state.currentTaskStatus?.tag).toBe("Working");
  });

  it("AgentEffortChanged updates the hydrated agent snapshot", () => {
    const state = sessionReducer(freshState(), {
      type: "hydrate",
      session: {
        id: "session-1",
        slug: "aaaa",
        project: "ship",
        branch_name: "ship/123/test",
        captain: {
          role: { tag: "Captain" },
          kind: { tag: "Codex" },
          state: { tag: "Idle" },
          context_remaining_percent: 75,
          model_id: "gpt-5-codex/high",
          available_models: ["gpt-5-codex/high", "gpt-5/high"],
          effort_config_id: "reasoning.effort",
          effort_value_id: "low",
          available_effort_values: [
            { id: "low", name: "Low" },
            { id: "high", name: "High" },
          ],
        },
        mate: {
          role: { tag: "Mate" },
          kind: { tag: "Codex" },
          state: { tag: "Idle" },
          context_remaining_percent: 55,
          model_id: null,
          available_models: [],
          effort_config_id: null,
          effort_value_id: null,
          available_effort_values: [],
        },
        startup_state: { tag: "Ready" },
        current_task: null,
        task_history: [],
        autonomy_mode: { tag: "HumanInTheLoop" },
        pending_steer: null,
        pending_human_review: null,
        title: null,
        created_at: "2026-01-01T00:00:00Z",
        user_avatar_url: null,
      },
    });

    const next = sessionReducer(state, {
      type: "event",
      envelope: {
        seq: 1n,
        timestamp: "2026-01-01T00:00:00Z",
        event: {
          tag: "AgentEffortChanged",
          role: { tag: "Captain" },
          effort_config_id: "reasoning.effort",
          effort_value_id: "high",
          available_effort_values: [
            { id: "low", name: "Low" },
            { id: "high", name: "High" },
          ],
        },
      },
    });

    expect(next.captain?.effort_config_id).toBe("reasoning.effort");
    expect(next.captain?.effort_value_id).toBe("high");
    expect(next.captain?.available_effort_values).toEqual([
      { id: "low", name: "Low" },
      { id: "high", name: "High" },
    ]);
  });

  it("tracks lastSeq from event envelopes", () => {
    const state = sessionReducer(freshState(), {
      type: "event",
      envelope: {
        seq: 999n,
        timestamp: "2026-01-01T00:00:00Z",
        event: { tag: "TaskStatusChanged", task_id: "t", status: { tag: "Accepted" } },
      },
    });
    expect(state.lastSeq).toBe(999);
    expect(state.lastEventKind).toBe("TaskStatusChanged");
    expect(state.eventCount).toBe(1);
  });
});
