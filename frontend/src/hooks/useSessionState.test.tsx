import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  SessionDetail,
  SessionEventEnvelope,
  ShipClient,
  SubscribeMessage,
} from "../generated/ship";
import { useSessionState } from "./useSessionState";

type TestTx<T> = {
  send(value: T): Promise<void>;
  close(): void;
};

type TestRx<T> = {
  recv(): Promise<T | null>;
};

const apiMocks = vi.hoisted(() => ({
  invalidateShipClientMock: vi.fn(),
  subscribeEventsMock: vi.fn(),
}));

vi.mock("@bearcove/roam-core", () => {
  function channel<T>(): [TestTx<T>, TestRx<T>] {
    const queue: Array<T | null> = [];
    const waiters: Array<(value: T | null) => void> = [];

    const push = (value: T | null) => {
      const waiter = waiters.shift();
      if (waiter) {
        waiter(value);
        return;
      }
      queue.push(value);
    };

    return [
      {
        async send(value: T) {
          push(value);
        },
        close() {
          push(null);
        },
      },
      {
        recv() {
          const next = queue.shift();
          if (next !== undefined) {
            return Promise.resolve(next);
          }
          return new Promise<T | null>((resolve) => {
            waiters.push(resolve);
          });
        },
      },
    ];
  }

  return { channel };
});

vi.mock("../api/client", () => ({
  getShipClient: async () =>
    ({
      subscribeEvents: apiMocks.subscribeEventsMock,
    }) as Pick<ShipClient, "subscribeEvents">,
  invalidateShipClient: apiMocks.invalidateShipClientMock,
}));

function SessionStateProbe({
  sessionId,
  session,
}: {
  sessionId: string;
  session: SessionDetail | null;
}) {
  const state = useSessionState(sessionId, session);

  return (
    <div
      data-testid="session-state"
      data-connected={String(state.connected)}
      data-disconnect-reason={state.disconnectReason ?? ""}
      data-event-count={String(state.eventCount)}
      data-phase={state.phase}
    >
      {state.currentTaskDescription ?? ""}
    </div>
  );
}

const session: SessionDetail = {
  id: "session-1",
  project: "ship",
  branch_name: "main",
  captain: {
    role: { tag: "Captain" },
    kind: { tag: "Claude" },
    state: { tag: "Idle" },
    context_remaining_percent: null,
  },
  mate: {
    role: { tag: "Mate" },
    kind: { tag: "Codex" },
    state: { tag: "Idle" },
    context_remaining_percent: null,
  },
  current_task: null,
  task_history: [],
  autonomy_mode: { tag: "HumanInTheLoop" },
  startup_state: { tag: "Ready" },
  pending_steer: null,
  created_at: "2026-01-01T00:00:00Z",
};

function taskStarted(seq: bigint, description: string): SessionEventEnvelope {
  return {
    seq,
    event: {
      tag: "TaskStarted",
      task_id: `task-${seq}`,
      description,
    },
  };
}

beforeEach(() => {
  apiMocks.subscribeEventsMock.mockReset();
  apiMocks.invalidateShipClientMock.mockReset();
  vi.spyOn(console, "debug").mockImplementation(() => undefined);
  vi.spyOn(console, "info").mockImplementation(() => undefined);
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useSessionState subscription lifecycle", () => {
  // r[verify event.client.connection-lifecycle]
  // r[verify event.subscribe]
  it("keeps consuming the subscription channel after subscribe setup resolves", async () => {
    let output: TestTx<SubscribeMessage> | null = null;
    let resolveSetup: (() => void) | null = null;

    apiMocks.subscribeEventsMock.mockImplementation(
      (_sessionId: string, tx: TestTx<SubscribeMessage>) =>
        new Promise<void>((resolve) => {
          output = tx;
          resolveSetup = resolve;
        }),
    );

    render(<SessionStateProbe sessionId="session-1" session={session} />);

    await waitFor(() => {
      expect(screen.getByTestId("session-state")).toHaveAttribute("data-phase", "replaying");
    });

    resolveSetup?.();

    await waitFor(() => {
      expect(screen.getByTestId("session-state")).toHaveAttribute("data-connected", "true");
      expect(apiMocks.invalidateShipClientMock).not.toHaveBeenCalled();
    });

    expect(output).not.toBeNull();

    await output!.send({ tag: "Event", value: taskStarted(10n, "Replay task") });
    await output!.send({ tag: "ReplayComplete" });
    await output!.send({
      tag: "Event",
      value: {
        seq: 11n,
        event: {
          tag: "TaskStatusChanged",
          task_id: "task-10",
          status: { tag: "Working" },
        },
      },
    });

    await waitFor(() => {
      const probe = screen.getByTestId("session-state");
      expect(probe).toHaveAttribute("data-phase", "live");
      expect(probe).toHaveAttribute("data-event-count", "2");
      expect(probe).toHaveTextContent("Replay task");
    });
  });

  // r[verify event.client.connection-lifecycle]
  it("still reports a real channel closure as a disconnect", async () => {
    let output: TestTx<SubscribeMessage> | null = null;

    apiMocks.subscribeEventsMock.mockImplementation(
      async (_sessionId: string, tx: TestTx<SubscribeMessage>) => {
        output = tx;
      },
    );

    render(<SessionStateProbe sessionId="session-1" session={session} />);

    await waitFor(() => {
      expect(screen.getByTestId("session-state")).toHaveAttribute("data-phase", "replaying");
    });

    expect(output).not.toBeNull();
    output!.close();

    await waitFor(() => {
      const probe = screen.getByTestId("session-state");
      expect(probe).toHaveAttribute("data-connected", "false");
      expect(probe).toHaveAttribute("data-disconnect-reason", "subscription channel closed");
    });
    expect(apiMocks.invalidateShipClientMock).toHaveBeenCalledWith("subscription channel closed");
  });

  // r[verify event.client.connection-lifecycle]
  it("does not reconnect while a healthy subscription remains open", async () => {
    let output: TestTx<SubscribeMessage> | null = null;

    apiMocks.subscribeEventsMock.mockImplementation(
      async (_sessionId: string, tx: TestTx<SubscribeMessage>) => {
        output = tx;
      },
    );

    render(<SessionStateProbe sessionId="session-1" session={session} />);

    await waitFor(() => {
      expect(screen.getByTestId("session-state")).toHaveAttribute("data-phase", "replaying");
    });

    await output!.send({ tag: "ReplayComplete" });

    await waitFor(() => {
      expect(screen.getByTestId("session-state")).toHaveAttribute("data-phase", "live");
    });

    vi.useFakeTimers();
    await vi.advanceTimersByTimeAsync(3_100);

    expect(apiMocks.subscribeEventsMock).toHaveBeenCalledTimes(1);
    expect(apiMocks.invalidateShipClientMock).not.toHaveBeenCalled();
    expect(screen.getByTestId("session-state")).toHaveAttribute("data-connected", "true");
    vi.useRealTimers();
  });

  // r[verify event.client.connection-lifecycle]
  it("reconnects after a real disconnect with a fresh subscription attempt", async () => {
    const outputs: TestTx<SubscribeMessage>[] = [];

    apiMocks.subscribeEventsMock.mockImplementation(
      async (_sessionId: string, tx: TestTx<SubscribeMessage>) => {
        outputs.push(tx);
      },
    );

    render(<SessionStateProbe sessionId="session-1" session={session} />);

    await waitFor(() => {
      expect(screen.getByTestId("session-state")).toHaveAttribute("data-phase", "replaying");
    });

    expect(outputs).toHaveLength(1);
    outputs[0]!.close();

    await waitFor(() => {
      expect(screen.getByTestId("session-state")).toHaveAttribute("data-connected", "false");
    });

    await new Promise((resolve) => window.setTimeout(resolve, 3_100));

    await waitFor(() => {
      expect(apiMocks.subscribeEventsMock).toHaveBeenCalledTimes(2);
    });

    expect(outputs).toHaveLength(2);
    await outputs[1]!.send({ tag: "ReplayComplete" });

    await waitFor(() => {
      const probe = screen.getByTestId("session-state");
      expect(probe).toHaveAttribute("data-connected", "true");
      expect(probe).toHaveAttribute("data-phase", "live");
    });
  }, 8_000);
});
