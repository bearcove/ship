import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, beforeEach, vi } from "vitest";
import type {
  AgentDiscovery,
  CreateSessionResponse,
  ProjectInfo,
  SessionSummary,
} from "../generated/ship";
import { renderWithTheme } from "../test/render";
import { SessionListPage } from "./SessionListPage";

const mocks = vi.hoisted(() => ({
  createSession: vi.fn<() => Promise<CreateSessionResponse>>(),
  refreshSessionList: vi.fn<() => Promise<SessionSummary[]>>(),
  addProject: vi.fn(),
  listAgentPresets: vi.fn<() => Promise<Array<{ id: string; label: string; kind: { tag: string }; provider: string; model_id: string }>>>(),
  projects: [] as ProjectInfo[],
  sessions: [] as SessionSummary[],
  discovery: { claude: true, codex: true, opencode: true } as AgentDiscovery,
  branchesByProject: {} as Record<string, string[]>,
  transcription: {
    state: { tag: "idle" } as
      | { tag: "idle" }
      | { tag: "recording"; elapsed: number }
      | { tag: "processing" },
    targetSessionId: null as string | null,
  },
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    createSession: mocks.createSession,
    addProject: mocks.addProject,
    listAgentPresets: mocks.listAgentPresets,
  }),
  onClientReady: () => () => {},
}));

vi.mock("../hooks/useProjects", () => ({
  useProjects: () => mocks.projects,
}));

vi.mock("../hooks/useSessionList", () => ({
  refreshSessionList: mocks.refreshSessionList,
  useSessionList: (projectFilter?: string) =>
    projectFilter
      ? mocks.sessions.filter((session) => session.project === projectFilter)
      : mocks.sessions,
}));

vi.mock("../hooks/useAgentDiscovery", () => ({
  useAgentDiscovery: () => mocks.discovery,
}));

vi.mock("../hooks/useBranches", () => ({
  useBranches: (projectName: string) => mocks.branchesByProject[projectName] ?? [],
}));

vi.mock("../context/TranscriptionContext", () => ({
  useTranscription: () => mocks.transcription,
}));

function makeSession(overrides: Partial<SessionSummary> = {}): SessionSummary {
  return {
    id: "session-1",
    slug: "aaaa",
    project: "ship",
    branch_name: "main",
    title: "Polish toolbar",
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
    startup_state: { tag: "Ready" },
    current_task_title: "Polish toolbar",
    current_task_description: "Polish the toolbar",
    task_status: { tag: "Working" },
    diff_stats: null,
    tasks_done: 0,
    tasks_total: 0,
    autonomy_mode: { tag: "HumanInTheLoop" },
    created_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function renderPage(entry = "/") {
  return renderWithTheme(
    <MemoryRouter initialEntries={[entry]}>
      <Routes>
        <Route path="/" element={<SessionListPage />} />
        <Route path="/sessions/:sessionId" element={<div>Session view</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.createSession.mockReset();
  mocks.refreshSessionList.mockReset();
  mocks.addProject.mockReset();
  mocks.listAgentPresets.mockReset();
  mocks.projects = [
    { name: "ship", path: "/tmp/ship", valid: true, invalid_reason: null },
    { name: "roam", path: "/tmp/roam", valid: true, invalid_reason: null },
  ];
  mocks.sessions = [];
  mocks.discovery = { claude: true, codex: true, opencode: true };
  mocks.branchesByProject = {
    ship: ["main", "release/2026.03"],
    roam: ["main"],
  };
  mocks.transcription = { state: { tag: "idle" }, targetSessionId: null };
  mocks.refreshSessionList.mockImplementation(async () => mocks.sessions);
  mocks.listAgentPresets.mockResolvedValue([]);
  mocks.createSession.mockResolvedValue({
    tag: "Created",
    session_id: "session-created",
  });
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("SessionListPage UX slice", () => {
  // r[verify ui.session-list.project-filter]
  it("keeps Add Project inside the project filter instead of a standalone toolbar button", async () => {
    mocks.sessions = [makeSession()];

    renderPage();

    expect(screen.getByRole("button", { name: /new session/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^add project$/i })).not.toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Filter projects"));

    const addProjectOption = await screen.findByRole("option", { name: "Add Project" });
    fireEvent.click(addProjectOption);

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Add Project")).toBeInTheDocument();
    expect(within(dialog).getByPlaceholderText("/absolute/path/to/repo")).toBeInTheDocument();
  });

  // r[verify ui.session-list.empty]
  // r[verify ui.session-list.project-filter]
  it("renders one filtered empty-state message without overlapping copy", () => {
    renderPage("/?project=ship");

    expect(screen.getByText("No sessions in ship yet.")).toBeInTheDocument();
    expect(screen.queryByText(/No sessions yet\./)).not.toBeInTheDocument();
    expect(screen.queryByText("No sessions in ship.")).not.toBeInTheDocument();
  });

  // r[verify ui.session-list.create]
  // r[verify ui.session-list.create.branch-filter]
  it("preselects the filtered project and submits once the branch is selected", async () => {
    renderPage("/?project=ship");

    fireEvent.click(screen.getAllByRole("button", { name: /new session/i })[0]);

    await screen.findByRole("dialog");

    const branchField = screen.getByLabelText("Base branch");
    fireEvent.focus(branchField);
    fireEvent.change(branchField, { target: { value: "release" } });
    fireEvent.mouseDown(await screen.findByRole("option", { name: "release/2026.03" }));

    fireEvent.click(screen.getByRole("button", { name: /create session/i }));

    await waitFor(() => {
      expect(mocks.createSession).toHaveBeenCalledWith({
        project: "ship",
        captain_kind: { tag: "Claude" },
        mate_kind: { tag: "Claude" },
        captain_preset_id: null,
        mate_preset_id: null,
        base_branch: "release/2026.03",
        mcp_servers: null,
      });
    });

    await waitFor(() => {
      expect(mocks.refreshSessionList).toHaveBeenCalledTimes(1);
    });

    await screen.findByText("Session view");
  });

  // r[verify ui.session-list.create.branch-filter]
  it("does not submit a stale branch when the combobox query is only a partial match", async () => {
    renderPage("/?project=ship");

    fireEvent.click(screen.getAllByRole("button", { name: /new session/i })[0]);

    await screen.findByRole("dialog");

    const branchField = screen.getByLabelText("Base branch");
    fireEvent.focus(branchField);
    fireEvent.change(branchField, { target: { value: "release" } });

    const createButton = screen.getByRole("button", { name: /create session/i });
    expect(createButton).toBeDisabled();

    expect(createButton).toBeDisabled();

    fireEvent.click(createButton);
    await waitFor(() => {
      expect(mocks.createSession).not.toHaveBeenCalled();
    });
  });

  it("shows a recording badge only on the session card that owns voice input", () => {
    mocks.sessions = [
      makeSession({ id: "session-1", slug: "aaaa", title: "Alpha" }),
      makeSession({ id: "session-2", slug: "bbbb", title: "Beta" }),
    ];
    mocks.transcription = {
      state: { tag: "recording", elapsed: 1200 },
      targetSessionId: "session-2",
    };

    renderPage();

    expect(screen.getByTestId("session-recording-badge-session-2")).toBeInTheDocument();
    expect(screen.queryByTestId("session-recording-badge-session-1")).not.toBeInTheDocument();
  });
});
