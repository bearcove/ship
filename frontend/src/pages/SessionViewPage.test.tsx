import { fireEvent, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { describe, expect, it, beforeEach, vi } from "vitest";
import type { SessionDetail } from "../generated/ship";
import { SoundProvider } from "../context/SoundContext";
import { renderWithTheme } from "../test/render";
import { SessionViewPage } from "./SessionViewPage";

const mocks = vi.hoisted(() => ({
  session: null as SessionDetail | null,
  eventState: {
    captain: null,
    mate: null,
    currentTaskId: null,
    currentTaskTitle: null,
    currentTaskDescription: null,
    currentTaskStatus: null,
    captainBlocks: { blocks: [] },
    mateBlocks: { blocks: [] },
    startupState: null,
    phase: "live" as const,
    connected: true,
    disconnectReason: null,
    replayEventCount: 0,
    connectionAttempt: 1,
    lastSeq: null,
    lastEventKind: null,
  },
  promptCaptain: vi.fn(),
  steer: vi.fn(),
}));

vi.mock("../hooks/useSession", () => ({
  useSession: () => mocks.session,
}));

vi.mock("../hooks/useSessionState", () => ({
  useSessionState: () => mocks.eventState,
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    getSession: async () => mocks.session,
    promptCaptain: mocks.promptCaptain,
    steer: mocks.steer,
    resolvePermission: async () => undefined,
    accept: async () => undefined,
    cancel: async () => undefined,
    listWorktreeFiles: async () => [],
  }),
}));

vi.mock("../components/ConnectionBanner", () => ({
  ConnectionBanner: () => null,
}));

function LocationEcho() {
  const location = useLocation();
  return <div>{`${location.pathname}${location.search}`}</div>;
}

function makeSession(): SessionDetail {
  return {
    id: "session-1",
    project: "roam",
    branch_name: "feature/breadcrumbs",
    captain: {
      role: { tag: "Captain" },
      kind: { tag: "Claude" },
      state: { tag: "Working", plan: null, activity: null },
      context_remaining_percent: 82,
      model_id: null,
      available_models: [],
    },
    mate: {
      role: { tag: "Mate" },
      kind: { tag: "Codex" },
      state: { tag: "Idle" },
      context_remaining_percent: 91,
      model_id: null,
      available_models: [],
    },
    current_task: {
      id: "task-1",
      title: "Tighten session chrome",
      description: "Tighten the session chrome",
      status: { tag: "ReviewPending" },
    },
    task_history: [],
    autonomy_mode: { tag: "HumanInTheLoop" },
    startup_state: { tag: "Ready" },
    pending_steer: null,
    created_at: "2026-01-01T00:00:00Z",
  };
}

function renderPage() {
  return renderWithTheme(
    <MemoryRouter initialEntries={["/sessions/session-1"]}>
      <SoundProvider>
        <Routes>
          <Route path="/" element={<LocationEcho />} />
          <Route path="/sessions/:sessionId" element={<SessionViewPage debugMode={false} />} />
        </Routes>
      </SoundProvider>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.session = makeSession();
  mocks.eventState = {
    captain: null,
    mate: null,
    currentTaskId: null,
    currentTaskTitle: null,
    currentTaskDescription: null,
    currentTaskStatus: null,
    captainBlocks: { blocks: [] },
    mateBlocks: { blocks: [] },
    startupState: null,
    phase: "live",
    connected: true,
    disconnectReason: null,
    replayEventCount: 0,
    connectionAttempt: 1,
    lastSeq: null,
    lastEventKind: null,
  };
  mocks.promptCaptain.mockReset();
  mocks.steer.mockReset();
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("SessionViewPage UX slice", () => {
  // r[verify view.session]
  // r[verify ui.layout.session-view]
  it("renders session view with agent panels", () => {
    renderPage();

    expect(screen.getAllByLabelText("Captain steer input").length).toBeGreaterThan(0);
    expect(screen.getAllByLabelText("Mate steer input").length).toBeGreaterThan(0);
  });

  // r[verify view.agent-panel.state]
  // r[verify ui.keys.steer-send]
  it("submits captain and mate inline steering from the feed footer with Enter", async () => {
    renderPage();

    expect(screen.queryByText("Claude")).not.toBeInTheDocument();
    expect(screen.queryByText("Codex")).not.toBeInTheDocument();

    const captainInput = screen.getAllByLabelText("Captain steer input")[0];
    const mateInput = screen.getAllByLabelText("Mate steer input")[0];

    fireEvent.change(captainInput, { target: { value: "Ask the captain to tighten the review." } });
    fireEvent.keyDown(captainInput, { key: "Enter" });

    await waitFor(() => {
      expect(mocks.promptCaptain).toHaveBeenCalledWith("session-1", [
        { tag: "Text", text: "Ask the captain to tighten the review." },
      ]);
    });

    fireEvent.change(mateInput, { target: { value: "Apply the captain notes directly." } });
    fireEvent.keyDown(mateInput, { key: "Enter" });

    await waitFor(() => {
      expect(mocks.steer).toHaveBeenCalledWith("session-1", [
        { tag: "Text", text: "Apply the captain notes directly." },
      ]);
    });
  });

  // r[verify ui.steer-review.layout]
  it("shows captain steer review without stale accept actions in steer-pending state", async () => {
    mocks.session = {
      ...makeSession(),
      current_task: {
        id: "task-1",
        title: "Ship captain workflow",
        description: "Ship the captain-led workflow",
        status: { tag: "SteerPending" },
      },
      pending_steer: "Tell the mate to tighten the review loop.",
    };

    renderPage();

    expect(screen.getByText("Captain's steer — awaiting your review")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send to Mate" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Accept mate work" })).not.toBeInTheDocument();
  });

  // r[verify view.session]
  it("renders startup progress in the captain feed instead of a page banner", () => {
    mocks.session = {
      ...makeSession(),
      startup_state: {
        tag: "Running",
        stage: { tag: "StartingCaptain" },
        message: "Starting captain (0.8s elapsed)",
      },
      current_task: null,
    };

    renderPage();

    expect(screen.getAllByText("Session startup").length).toBeGreaterThan(0);
    expect(screen.queryByText("Session startup is in progress.")).not.toBeInTheDocument();
  });
});
