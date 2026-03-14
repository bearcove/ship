import { fireEvent, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, beforeEach, vi } from "vitest";
import type { SessionDetail } from "../generated/ship";
import { SoundProvider } from "../context/SoundContext";
import { initialSessionViewState } from "../state/sessionReducer";
import { renderWithTheme } from "../test/render";
import { SessionViewPage } from "./SessionViewPage";

const mocks = vi.hoisted(() => ({
  session: null as SessionDetail | null,
  sessionError: null as string | null,
  eventState: {
    captain: null,
    mate: null,
    captainAcpInfo: null,
    mateAcpInfo: null,
    captainBlocks: { blocks: [], index: new Map() },
    mateBlocks: { blocks: [], index: new Map() },
    unifiedBlocks: { blocks: [], index: new Map() },
    startupState: null,
    currentTaskId: null,
    currentTaskTitle: null,
    currentTaskDescription: null,
    currentTaskStatus: null,
    currentTaskStartedAt: null,
    currentTaskCompletedAt: null,
    captainTurnStartedAt: null,
    mateTurnStartedAt: null,
    currentTaskSteps: [],
    phase: "live" as const,
    connected: true,
    lastSeq: null,
    lastEventKind: null,
    eventCount: 0,
    replayEventCount: 0,
    disconnectReason: null,
    connectionAttempt: 1,
    pendingHumanReview: null,
    title: null,
  },
  promptCaptain: vi.fn(),
  steer: vi.fn(),
  transcription: {
    state: { tag: "idle" as const },
    result: null,
    analyser: null,
    targetSessionId: null as string | null,
    sendAfterTranscription: false,
    startRecording: vi.fn(),
    stopRecording: vi.fn(),
    stopAndSend: vi.fn(),
    cancelRecording: vi.fn(),
    clearResult: vi.fn(),
    isRecording: vi.fn(() => false),
  },
}));

vi.mock("../hooks/useSession", () => ({
  useSession: () => ({ session: mocks.session, error: mocks.sessionError }),
}));

vi.mock("../hooks/useSessionState", () => ({
  useSessionState: () => mocks.eventState,
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    getSession: async () => mocks.session,
    agentDiscovery: async () => ({ claude: true, codex: true, opencode: true }),
    promptCaptain: mocks.promptCaptain,
    steer: mocks.steer,
    resolvePermission: async () => undefined,
    accept: async () => undefined,
    cancel: async () => undefined,
    interruptCaptain: async () => undefined,
    listWorktreeFiles: async () => [],
  }),
  onClientReady: () => () => {},
}));

vi.mock("../components/ConnectionBanner", () => ({
  ConnectionBanner: () => null,
}));

vi.mock("../context/TranscriptionContext", () => ({
  useTranscription: () => mocks.transcription,
}));

function makeSession(): SessionDetail {
  return {
    id: "session-1",
    slug: "aaaa",
    project: "roam",
    branch_name: "feature/breadcrumbs",
    title: null,
    captain: {
      role: { tag: "Captain" },
      kind: { tag: "Claude" },
      state: { tag: "Working", plan: null, activity: null },
      context_remaining_percent: 82,
      preset_id: null,
      provider: null,
      model_id: null,
      available_models: [],
      effort_config_id: null,
      effort_value_id: null,
      available_effort_values: [],
    },
    mate: {
      role: { tag: "Mate" },
      kind: { tag: "Codex" },
      state: { tag: "Idle" },
      context_remaining_percent: 91,
      preset_id: null,
      provider: null,
      model_id: null,
      available_models: [],
      effort_config_id: null,
      effort_value_id: null,
      available_effort_values: [],
    },
    current_task: {
      id: "task-1",
      title: "Tighten session chrome",
      description: "Tighten the session chrome",
      status: { tag: "ReviewPending" },
      steps: [],
      assigned_at: null,
      completed_at: null,
    },
    task_history: [],
    autonomy_mode: { tag: "HumanInTheLoop" },
    startup_state: { tag: "Ready" },
    pending_steer: null,
    pending_human_review: null,
    created_at: "2026-01-01T00:00:00Z",
    user_avatar_url: null,
    captain_acp_info: null,
    mate_acp_info: null,
  };
}

function renderPage(initialEntries: string[] = ["/sessions/session-1"]) {
  return renderWithTheme(
    <MemoryRouter initialEntries={initialEntries}>
      <SoundProvider>
        <Routes>
          <Route path="/" element={<div>home</div>} />
          <Route path="/sessions/:sessionId" element={null} />
        </Routes>
        <SessionViewPage sessionId="session-1" isActive={true} debugMode={false} />
      </SoundProvider>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.session = makeSession();
  mocks.sessionError = null;
  mocks.eventState = {
    ...initialSessionViewState(),
    captain: null,
    mate: null,
    phase: "live",
    connected: true,
    connectionAttempt: 1,
  };
  mocks.promptCaptain.mockReset();
  mocks.steer.mockReset();
  mocks.transcription = {
    state: { tag: "idle" },
    result: null,
    analyser: null,
    targetSessionId: null,
    sendAfterTranscription: false,
    startRecording: vi.fn(),
    stopRecording: vi.fn(),
    stopAndSend: vi.fn(),
    cancelRecording: vi.fn(),
    clearResult: vi.fn(),
    isRecording: vi.fn(() => false),
  };
  URL.createObjectURL = vi.fn(() => "blob:session-view-test");
  URL.revokeObjectURL = vi.fn();
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("SessionViewPage UX slice", () => {
  // r[verify view.session]
  // r[verify ui.layout.session-view]
  it("renders session view with agent panels", () => {
    renderPage();

    expect(screen.getByLabelText("Steer input")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Switch session" })).toBeInTheDocument();
  });

  it("shows the composer drop indicator and attaches dropped images from the feed", async () => {
    renderPage();

    const textarea = screen.getByLabelText("Steer input");
    const feed = screen.getByTestId("session-feed-drop-target");
    const file = new File(["image-bytes"], "drop.png", { type: "image/png" });

    expect(screen.queryByTestId("composer-drop-indicator")).not.toBeInTheDocument();

    fireEvent.dragEnter(feed, { dataTransfer: { files: [file] } });
    expect(screen.getByTestId("composer-drop-indicator")).toBeInTheDocument();

    fireEvent.drop(feed, { dataTransfer: { files: [file] } });

    expect(textarea).toHaveFocus();
    await waitFor(() => {
      expect(screen.getByAltText("drop.png")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("composer-drop-indicator")).not.toBeInTheDocument();
  });


  it("focuses the composer with c but ignores modifier shortcuts and active inputs", () => {
    renderPage();

    const textarea = screen.getByLabelText("Steer input");
    const switchButton = screen.getByRole("button", { name: "Switch session" });
    switchButton.focus();

    fireEvent.keyDown(window, { key: "c" });
    expect(textarea).toHaveFocus();

    switchButton.focus();
    fireEvent.keyDown(window, { key: "c", metaKey: true });
    expect(switchButton).toHaveFocus();

    const outsideInput = document.createElement("input");
    document.body.appendChild(outsideInput);
    outsideInput.focus();

    fireEvent.keyDown(outsideInput, { key: "c" });
    expect(outsideInput).toHaveFocus();
    expect(textarea).not.toHaveFocus();

    outsideInput.remove();
  });

  // r[verify view.agent-panel.state]
  // r[verify ui.keys.steer-send]
  it("submits captain and mate inline steering from the feed footer with Enter", async () => {
    mocks.session = {
      ...makeSession(),
      captain: {
        ...makeSession().captain,
        state: { tag: "Idle" },
      },
    };

    renderPage();

    const input = screen.getByLabelText("Steer input");

    fireEvent.change(input, { target: { value: "Ask the captain to tighten the review." } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(mocks.promptCaptain).toHaveBeenCalledWith("session-1", [
        { tag: "Text", text: "Ask the captain to tighten the review." },
      ]);
    });

    fireEvent.change(input, { target: { value: "@mate Apply the captain notes directly." } });
    fireEvent.keyDown(input, { key: "Enter" });

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
        steps: [],
        assigned_at: null,
        completed_at: null,
      },
      pending_steer: "Tell the mate to tighten the review loop.",
    };

    renderPage();

    expect(screen.getByText("Captain's steer — awaiting your review")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send to Mate" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Accept mate work" })).not.toBeInTheDocument();
  });

  // r[verify view.session]
  it("hides archive controls until the session has accepted work", () => {
    renderPage();

    expect(screen.queryByRole("button", { name: "Archive session" })).not.toBeInTheDocument();
  });

  // r[verify view.session]
  it("shows archive controls once the session has accepted work", () => {
    mocks.session = {
      ...makeSession(),
      current_task: null,
      task_history: [
        {
          id: "task-0",
          title: "Land session view archive gating",
          description: "Ship the archive gating change.",
          status: { tag: "Accepted" },
          steps: [],
          assigned_at: null,
          completed_at: "2026-03-13T10:00:00Z",
        },
      ],
    };

    renderPage();

    expect(screen.getByRole("button", { name: "Archive session" })).toBeInTheDocument();
  });

  it("pressing R with a text selection inserts a blockquote into the composer", () => {
    renderPage();

    // Mock window.getSelection to return a selection with HTML content
    const fragment = document.createDocumentFragment();
    const bold = document.createElement("strong");
    bold.textContent = "important";
    fragment.appendChild(bold);

    const mockRange = {
      cloneContents: () => fragment,
    };
    const mockSelection = {
      isCollapsed: false,
      rangeCount: 1,
      getRangeAt: () => mockRange,
      removeAllRanges: vi.fn(),
      toString: () => "important",
    };
    vi.spyOn(window, "getSelection").mockReturnValue(mockSelection as unknown as Selection);

    fireEvent.keyDown(window, { key: "r" });

    const textarea = screen.getByLabelText("Steer input");
    // turndown converts <strong>important</strong> to **important**
    expect(textarea).toHaveValue("> **important**\n\n");
    expect(mockSelection.removeAllRanges).toHaveBeenCalled();

    vi.restoreAllMocks();
  });

  it("pressing R without a text selection does nothing", () => {
    renderPage();

    vi.spyOn(window, "getSelection").mockReturnValue({
      isCollapsed: true,
      rangeCount: 0,
      toString: () => "",
    } as unknown as Selection);

    const textarea = screen.getByLabelText("Steer input");
    fireEvent.keyDown(window, { key: "r" });
    expect(textarea).toHaveValue("");

    vi.restoreAllMocks();
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
