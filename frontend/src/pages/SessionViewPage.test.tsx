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
    currentTaskDescription: null,
    currentTaskStatus: null,
    captainBlocks: { blocks: [] },
    mateBlocks: { blocks: [] },
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
    assign: async () => "task-2",
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
    },
    mate: {
      role: { tag: "Mate" },
      kind: { tag: "Codex" },
      state: { tag: "Idle" },
      context_remaining_percent: 91,
    },
    current_task: {
      id: "task-1",
      description: "Tighten the session chrome",
      status: { tag: "ReviewPending" },
    },
    task_history: [],
    autonomy_mode: { tag: "HumanInTheLoop" },
    pending_steer: null,
  };
}

function renderPage() {
  return renderWithTheme(
    <MemoryRouter initialEntries={["/sessions/session-1"]}>
      <SoundProvider>
        <Routes>
          <Route path="/" element={<LocationEcho />} />
          <Route path="/sessions/:sessionId" element={<SessionViewPage />} />
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
    currentTaskDescription: null,
    currentTaskStatus: null,
    captainBlocks: { blocks: [] },
    mateBlocks: { blocks: [] },
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
  // r[verify ui.notify.sound-toggle]
  // r[verify ui.autonomy.toggle]
  it("renders breadcrumb session chrome with project navigation and the session mute control", async () => {
    renderPage();

    expect(screen.getByRole("button", { name: "ship" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "roam" })).toBeInTheDocument();
    expect(screen.getByText("feature/breadcrumbs")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /close session/i })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /mute sounds/i })).toBeInTheDocument();
    expect(screen.getByText("Human-in-the-loop")).toBeInTheDocument();
    expect(screen.queryByText("Autonomous")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "roam" }));

    expect(await screen.findByText("/?project=roam")).toBeInTheDocument();
  });

  // r[verify view.agent-panel.state]
  // r[verify ui.keys.steer-send]
  it("submits captain and mate inline steering from the feed footer with Cmd+Enter", async () => {
    renderPage();

    expect(screen.queryByText("Claude")).not.toBeInTheDocument();
    expect(screen.queryByText("Codex")).not.toBeInTheDocument();

    const captainInput = screen.getAllByLabelText("Captain steer input")[0];
    const mateInput = screen.getAllByLabelText("Mate steer input")[0];

    expect(screen.getAllByText("Working").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Idle").length).toBeGreaterThan(0);

    fireEvent.change(captainInput, { target: { value: "Ask the captain to tighten the review." } });
    fireEvent.keyDown(captainInput, { key: "Enter", metaKey: true });

    await waitFor(() => {
      expect(mocks.promptCaptain).toHaveBeenCalledWith(
        "session-1",
        "Ask the captain to tighten the review.",
      );
    });

    fireEvent.change(mateInput, { target: { value: "Apply the captain notes directly." } });
    fireEvent.keyDown(mateInput, { key: "Enter", metaKey: true });

    await waitFor(() => {
      expect(mocks.steer).toHaveBeenCalledWith("session-1", "Apply the captain notes directly.");
    });
  });
});
