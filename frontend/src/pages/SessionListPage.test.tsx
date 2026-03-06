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
  addProject: vi.fn(),
  projects: [] as ProjectInfo[],
  sessions: [] as SessionSummary[],
  discovery: { claude: true, codex: true } as AgentDiscovery,
  branchesByProject: {} as Record<string, string[]>,
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    createSession: mocks.createSession,
    addProject: mocks.addProject,
  }),
}));

vi.mock("../hooks/useProjects", () => ({
  useProjects: () => mocks.projects,
}));

vi.mock("../hooks/useSessionList", () => ({
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

function makeSession(overrides: Partial<SessionSummary> = {}): SessionSummary {
  return {
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
    current_task_description: "Polish the toolbar",
    task_status: { tag: "Working" },
    autonomy_mode: { tag: "HumanInTheLoop" },
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
  mocks.addProject.mockReset();
  mocks.projects = [
    { name: "ship", path: "/tmp/ship", valid: true, invalid_reason: null },
    { name: "roam", path: "/tmp/roam", valid: true, invalid_reason: null },
  ];
  mocks.sessions = [];
  mocks.discovery = { claude: true, codex: true };
  mocks.branchesByProject = {
    ship: ["main", "release/2026.03"],
    roam: ["main"],
  };
  mocks.createSession.mockResolvedValue({
    tag: "Created",
    session_id: "session-created",
    task_id: "task-created",
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

    expect(
      screen.getByText("No sessions in ship yet. Start one for this project."),
    ).toBeInTheDocument();
    expect(screen.queryByText(/No sessions yet\./)).not.toBeInTheDocument();
    expect(screen.queryByText("No sessions in ship.")).not.toBeInTheDocument();
  });

  // r[verify ui.session-list.create]
  // r[verify ui.session-list.create.branch-filter]
  it("preselects the filtered project and submits via Cmd+Enter from the task field", async () => {
    renderPage("/?project=ship");

    fireEvent.click(screen.getAllByRole("button", { name: /new session/i })[0]);

    await screen.findByRole("dialog");
    expect(screen.getByRole("button", { name: /create session/i })).toHaveTextContent("⌘");
    expect(screen.getByRole("button", { name: /create session/i })).toHaveTextContent("↩");

    const branchField = screen.getByLabelText("Base branch");
    fireEvent.focus(branchField);
    fireEvent.change(branchField, { target: { value: "release" } });
    fireEvent.mouseDown(await screen.findByRole("option", { name: "release/2026.03" }));

    const taskField = screen.getByLabelText("Task description");
    fireEvent.change(taskField, { target: { value: "Tighten the new-session flow" } });
    fireEvent.keyDown(taskField, { key: "Enter", metaKey: true });

    await waitFor(() => {
      expect(mocks.createSession).toHaveBeenCalledWith({
        project: "ship",
        captain_kind: { tag: "Claude" },
        mate_kind: { tag: "Claude" },
        base_branch: "release/2026.03",
        task_description: "Tighten the new-session flow",
        mcp_servers: null,
      });
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

    const taskField = screen.getByLabelText("Task description");
    fireEvent.change(taskField, { target: { value: "Tighten the new-session flow" } });
    expect(createButton).toBeDisabled();

    fireEvent.click(createButton);
    await waitFor(() => {
      expect(mocks.createSession).not.toHaveBeenCalled();
    });
  });
});
