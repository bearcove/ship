import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentPreset, AgentSnapshot, PlanStep, TaskRecord } from "../generated/ship";
import { resetAgentPresetsForTest } from "../hooks/useAgentPresets";
import { renderWithTheme } from "../test/render";
import { SessionHeader } from "./SessionHeader";

const apiMocks = vi.hoisted(() => ({
  listAgentPresets: vi.fn<() => Promise<AgentPreset[]>>(async () => []),
  setAgentPreset: vi.fn<
    () => Promise<
      | { tag: "Ok" }
      | { tag: "Failed"; message: string }
      | { tag: "AgentNotSpawned" }
      | { tag: "SessionNotFound" }
      | { tag: "PresetNotFound" }
    >
  >(async () => ({ tag: "Ok" })),
  openInEditor: vi.fn(async () => undefined),
  openInTerminal: vi.fn(async () => undefined),
  navigate: vi.fn(),
  onClientReady: vi.fn(() => () => { }),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    listAgentPresets: apiMocks.listAgentPresets,
    setAgentPreset: apiMocks.setAgentPreset,
    openInEditor: apiMocks.openInEditor,
    openInTerminal: apiMocks.openInTerminal,
  }),
  onClientReady: apiMocks.onClientReady,
}));

vi.mock("react-router-dom", () => ({
  useNavigate: () => apiMocks.navigate,
}));

vi.mock("../hooks/useSessionList", () => ({
  useSessionList: () => [],
}));

vi.mock("../pages/SessionListPage", () => ({
  NewSessionDialog: () => null,
}));

const configuredPresets: AgentPreset[] = [
  {
    id: "preset-captain-claude",
    label: "Captain Claude",
    kind: { tag: "Claude" },
    provider: "anthropic",
    model_id: "opencode/anthropic/claude-sonnet-4",
    logo: null,
  },
  {
    id: "preset-mate-gpt",
    label: "Mate GPT-5",
    kind: { tag: "Codex" },
    provider: "openai",
    model_id: "opencode/openai/gpt-5",
    logo: null,
  },
  {
    id: "preset-haiku",
    label: "Claude Haiku",
    kind: { tag: "Claude" },
    provider: "anthropic",
    model_id: "opencode/anthropic/claude-haiku-4.5",
    logo: null,
  },
];

function makeAgent(role: AgentSnapshot["role"], overrides: Partial<AgentSnapshot> = {}): AgentSnapshot {
  const isCaptain = role.tag === "Captain";
  return {
    role,
    kind: isCaptain ? { tag: "Claude" } : { tag: "Codex" },
    state: { tag: "Working", plan: null, activity: null },
    context_remaining_percent: 76,
    preset_id: isCaptain ? "preset-captain-claude" : "preset-mate-gpt",
    provider: isCaptain ? "anthropic" : "openai",
    model_id: isCaptain ? "opencode/anthropic/claude-sonnet-4" : "opencode/openai/gpt-5",
    available_models: [
      "opencode/anthropic/claude-sonnet-4",
      "opencode/anthropic/claude-haiku-4.5",
      "opencode/openai/gpt-5",
    ],
    effort_config_id: null,
    effort_value_id: null,
    available_effort_values: [],
    ...overrides,
  };
}

function makeTask(overrides: Partial<TaskRecord> = {}): TaskRecord {
  return {
    id: "task-1",
    title: "Fix preset switching",
    description: "Verify preset switching from the session header",
    status: { tag: "Working" },
    steps: [],
    assigned_at: null,
    completed_at: null,
    ...overrides,
  };
}

function renderHeader({
  captain = makeAgent({ tag: "Captain" }),
  mate = makeAgent({ tag: "Mate" }),
  planSteps = [] as PlanStep[],
}: {
  captain?: AgentSnapshot | null;
  mate?: AgentSnapshot | null;
  planSteps?: PlanStep[];
} = {}) {
  return renderWithTheme(
    <SessionHeader
      sessionId="session-1"
      project="ship"
      title="Session title"
      branchName="feature/presets"
      captain={captain}
      mate={mate}
      liveTask={makeTask()}
      taskHistory={[]}
      planSteps={planSteps}
      matePlan={null}
      diffStats={null}
      checksState={null}
      onArchive={() => { }}
      archiving={false}
    />,
  );
}

beforeEach(() => {
  resetAgentPresetsForTest();
  apiMocks.listAgentPresets.mockReset();
  apiMocks.listAgentPresets.mockResolvedValue(configuredPresets);
  apiMocks.setAgentPreset.mockReset();
  apiMocks.setAgentPreset.mockResolvedValue({ tag: "Ok" });
  apiMocks.openInEditor.mockReset();
  apiMocks.openInEditor.mockResolvedValue(undefined);
  apiMocks.openInTerminal.mockReset();
  apiMocks.openInTerminal.mockResolvedValue(undefined);
  apiMocks.navigate.mockReset();
  apiMocks.onClientReady.mockReset();
  apiMocks.onClientReady.mockReturnValue(() => { });
});

describe("SessionHeader", () => {
  it("renders discovered preset labels in the expanded agents row and switches presets from that surface", async () => {
    renderHeader();

    fireEvent.click(screen.getByText("Fix preset switching"));

    expect(await screen.findByText("Captain Claude")).toBeInTheDocument();
    expect(await screen.findByText("Mate GPT-5")).toBeInTheDocument();
    expect(screen.getByText("opencode/anthropic/claude-sonnet-4")).toBeInTheDocument();
    expect(screen.getByText("opencode/openai/gpt-5")).toBeInTheDocument();

    const presetButtons = await screen.findAllByRole("button", { name: "Select preset" });
    fireEvent.click(presetButtons[1]!);

    const searchInput = await screen.findByLabelText("Search presets");
    fireEvent.change(searchInput, { target: { value: "haiku" } });
    fireEvent.mouseDown(await screen.findByRole("option", { name: /Claude Haiku/i }));

    await waitFor(() => {
      expect(apiMocks.setAgentPreset).toHaveBeenCalledWith(
        "session-1",
        { tag: "Mate" },
        "preset-haiku",
      );
    });
  });
});
