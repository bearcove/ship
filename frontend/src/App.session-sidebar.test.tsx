import { fireEvent, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AgentDiscovery,
  CreateSessionResponse,
  ProjectInfo,
  SessionSummary,
} from "./generated/ship";
import { App } from "./App";
import { SoundProvider } from "./context/SoundContext";
import { renderWithTheme } from "./test/render";

const mocks = vi.hoisted(() => ({
  createSession: vi.fn<() => Promise<CreateSessionResponse>>(),
  listSessions: vi.fn<() => Promise<SessionSummary[]>>(),
  projects: [] as ProjectInfo[],
  discovery: { claude: true, codex: true } as AgentDiscovery,
  branchesByProject: {} as Record<string, string[]>,
  sessions: [] as SessionSummary[],
}));

vi.mock("./api/client", () => ({
  getShipClient: async () => ({
    createSession: mocks.createSession,
    listSessions: mocks.listSessions,
  }),
}));

vi.mock("./components/ConnectionBanner", () => ({
  ConnectionBanner: () => null,
}));

vi.mock("./components/NotificationPrompt", () => ({
  NotificationPrompt: () => null,
}));

vi.mock("./hooks/useProjects", () => ({
  useProjects: () => mocks.projects,
}));

vi.mock("./hooks/useAgentDiscovery", () => ({
  useAgentDiscovery: () => mocks.discovery,
}));

vi.mock("./hooks/useBranches", () => ({
  useBranches: (projectName: string) => mocks.branchesByProject[projectName] ?? [],
}));

vi.mock("./pages/SessionViewPage", () => ({
  SessionViewPage: () => <div>Session view page</div>,
}));

function makeSession(overrides: Partial<SessionSummary> = {}): SessionSummary {
  return {
    id: "session-created",
    project: "ship",
    branch_name: "main",
    captain: {
      role: { tag: "Captain" },
      kind: { tag: "Claude" },
      state: { tag: "Idle" },
      context_remaining_percent: null,
      model_id: null,
      available_models: [],
    },
    mate: {
      role: { tag: "Mate" },
      kind: { tag: "Claude" },
      state: { tag: "Idle" },
      context_remaining_percent: null,
      model_id: null,
      available_models: [],
    },
    startup_state: { tag: "Ready" },
    current_task_description: "Polish the toolbar",
    task_status: null,
    autonomy_mode: { tag: "HumanInTheLoop" },
    created_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

beforeEach(() => {
  window.localStorage.clear();
  mocks.sessions = [];
  mocks.projects = [{ name: "ship", path: "/tmp/ship", valid: true, invalid_reason: null }];
  mocks.discovery = { claude: true, codex: true };
  mocks.branchesByProject = { ship: ["main"] };
  mocks.listSessions.mockReset();
  mocks.listSessions.mockImplementation(async () => mocks.sessions);
  mocks.createSession.mockReset();
  mocks.createSession.mockImplementation(async () => {
    mocks.sessions = [makeSession()];
    return {
      tag: "Created",
      session_id: "session-created",
    };
  });
});

// r[verify ui.layout.shell]
describe("App session sidebar refresh", () => {
  it("shows the sidebar on the first navigation after creating the first session", async () => {
    renderWithTheme(
      <MemoryRouter initialEntries={["/"]}>
        <SoundProvider>
          <App />
        </SoundProvider>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(mocks.listSessions).toHaveBeenCalledTimes(1);
    });

    expect(screen.queryByRole("link", { current: "page" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /new session/i }));

    const createButton = await screen.findByRole("button", { name: /create session/i });
    await waitFor(() => {
      expect(createButton).toBeEnabled();
    });

    fireEvent.click(createButton);

    expect(await screen.findByText("Session view page")).toBeInTheDocument();

    await waitFor(() => {
      expect(mocks.listSessions).toHaveBeenCalledTimes(2);
    });

    expect(await screen.findByRole("link", { current: "page" })).toHaveAttribute(
      "href",
      "/sessions/session-created",
    );
  });
});
