import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentPreset, AgentSnapshot } from "../generated/ship";
import { resetAgentPresetsForTest } from "../hooks/useAgentPresets";
import { renderWithTheme } from "../test/render";
import {
  agentHeaderAvatar,
  agentHeaderBody,
  agentHeaderContext,
  agentHeaderControlRow,
  agentHeaderSummaryRow,
} from "../styles/session-view.css";
import { AgentHeader } from "./AgentHeader";

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
  retryAgent: vi.fn(async () => undefined),
  onClientReady: vi.fn(() => () => {}),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    listAgentPresets: apiMocks.listAgentPresets,
    setAgentPreset: apiMocks.setAgentPreset,
    retryAgent: apiMocks.retryAgent,
  }),
  onClientReady: apiMocks.onClientReady,
}));

const configuredPresets: AgentPreset[] = [
  {
    id: "preset-claude-sonnet",
    label: "Claude Sonnet",
    kind: { tag: "Claude" },
    provider: "anthropic",
    model_id: "opencode/anthropic/claude-sonnet-4",
  },
  {
    id: "preset-gpt-5",
    label: "GPT-5 Turbo",
    kind: { tag: "Codex" },
    provider: "openai",
    model_id: "opencode/openai/gpt-5",
  },
  {
    id: "preset-haiku",
    label: "Claude Haiku",
    kind: { tag: "Claude" },
    provider: "anthropic",
    model_id: "opencode/anthropic/claude-haiku-4.5",
  },
];

function makeAgent(overrides: Partial<AgentSnapshot> = {}): AgentSnapshot {
  return {
    role: { tag: "Captain" },
    kind: { tag: "Claude" },
    state: { tag: "Working", plan: null, activity: null },
    context_remaining_percent: 82,
    preset_id: "preset-claude-sonnet",
    provider: "anthropic",
    model_id: "opencode/anthropic/claude-sonnet-4",
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

function renderHeader(agent: AgentSnapshot, avatarSrc?: string) {
  return renderWithTheme(<AgentHeader sessionId="session-1" agent={agent} avatarSrc={avatarSrc} />);
}

beforeEach(() => {
  resetAgentPresetsForTest();
  apiMocks.listAgentPresets.mockReset();
  apiMocks.listAgentPresets.mockResolvedValue(configuredPresets);
  apiMocks.setAgentPreset.mockReset();
  apiMocks.setAgentPreset.mockResolvedValue({ tag: "Ok" });
  apiMocks.retryAgent.mockReset();
  apiMocks.retryAgent.mockResolvedValue(undefined);
  apiMocks.onClientReady.mockReset();
  apiMocks.onClientReady.mockReturnValue(() => {});
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("AgentHeader", () => {
  // r[verify ui.agent-header.layout]
  // r[verify view.agent-panel.state]
  it("renders the discovered preset label and current model alongside the circular context indicator", async () => {
    const { container } = renderHeader(makeAgent(), "/captain.png");

    expect(container.querySelector(`img.${agentHeaderAvatar}[alt="Captain"]`)).toBeInTheDocument();

    const headerBody = container.querySelector<HTMLElement>(`.${agentHeaderBody}`);
    const summaryRow = container.querySelector<HTMLElement>(`.${agentHeaderSummaryRow}`);
    const controlRow = container.querySelector<HTMLElement>(`.${agentHeaderControlRow}`);

    expect(headerBody).toBeInTheDocument();
    expect(summaryRow).toBeInTheDocument();
    expect(controlRow).toBeInTheDocument();
    expect(summaryRow?.nextElementSibling).toBe(controlRow);

    const progressbar = screen.getByRole("progressbar", { name: "Captain context remaining" });
    expect(progressbar).toHaveClass(agentHeaderContext);
    expect(progressbar).toHaveAttribute("aria-valuenow", "82");
    expect(progressbar.querySelectorAll("circle")).toHaveLength(2);
    expect(progressbar).not.toHaveTextContent("82");

    expect(await within(controlRow!).findByText("Claude Sonnet")).toBeInTheDocument();
    expect(within(controlRow!).getByText("opencode/anthropic/claude-sonnet-4")).toBeInTheDocument();
  });

  // r[verify ui.agent-header.layout]
  it("filters configured presets from the server and calls setAgentPreset with the selected id", async () => {
    renderHeader(makeAgent());

    fireEvent.click(await screen.findByRole("button", { name: "Select preset" }));

    const searchInput = await screen.findByLabelText("Search presets");
    expect(screen.getByRole("option", { name: /Claude Sonnet/i })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /GPT-5 Turbo/i })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Claude Haiku/i })).toBeInTheDocument();

    fireEvent.change(searchInput, { target: { value: "gpt" } });

    await waitFor(() => {
      expect(screen.getByRole("option", { name: /GPT-5 Turbo/i })).toBeInTheDocument();
    });
    expect(screen.queryByRole("option", { name: /Claude Sonnet/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Claude Haiku/i })).not.toBeInTheDocument();

    fireEvent.mouseDown(screen.getByRole("option", { name: /GPT-5 Turbo/i }));

    await waitFor(() => {
      expect(apiMocks.setAgentPreset).toHaveBeenCalledWith(
        "session-1",
        { tag: "Captain" },
        "preset-gpt-5",
      );
    });
  });

  // r[verify ui.agent-header.layout]
  it("renders static preset text when the only configured preset already matches the current agent", async () => {
    apiMocks.listAgentPresets.mockResolvedValueOnce([configuredPresets[0]!]);

    renderHeader(makeAgent());

    expect(await screen.findByText("Claude Sonnet")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Select preset" })).not.toBeInTheDocument();
  });

  it("treats an inferred preset as the active selection when preset_id is still null", async () => {
    renderHeader(makeAgent({ preset_id: null }));

    fireEvent.click(await screen.findByRole("button", { name: "Select preset" }));

    const activeOption = await screen.findByRole("option", { name: /Claude Sonnet/i });
    expect(activeOption).toHaveAttribute("aria-selected", "true");

    fireEvent.mouseDown(activeOption);

    expect(apiMocks.setAgentPreset).not.toHaveBeenCalled();
  });

  // r[verify ui.error.agent]
  it("surfaces failed setAgentPreset errors without losing the current selection label", async () => {
    apiMocks.setAgentPreset.mockResolvedValueOnce({
      tag: "Failed",
      message: "Provider unavailable",
    });

    renderHeader(makeAgent());

    fireEvent.click(await screen.findByRole("button", { name: "Select preset" }));
    fireEvent.mouseDown(await screen.findByRole("option", { name: /GPT-5 Turbo/i }));

    await waitFor(() => {
      expect(screen.getByText("Provider unavailable")).toBeInTheDocument();
    });
    expect(screen.getByText("Claude Sonnet")).toBeInTheDocument();
  });

  // r[verify ui.agent-header.context-bar]
  // r[verify context.warning]
  // r[verify context.manual-rotation]
  it("keeps the low-context warning below the header and hides the donut once context is exhausted", () => {
    const { rerender } = renderWithTheme(
      <AgentHeader
        sessionId="session-1"
        agent={makeAgent({
          context_remaining_percent: 18,
          state: { tag: "Working", plan: null, activity: null },
        })}
      />,
    );

    expect(screen.getByRole("progressbar", { name: "Captain context remaining" })).toHaveAttribute(
      "aria-valuenow",
      "18",
    );
    expect(screen.getByText(/Context window below 20%/)).toBeInTheDocument();

    rerender(
      <AgentHeader
        sessionId="session-1"
        agent={makeAgent({ context_remaining_percent: 0, state: { tag: "ContextExhausted" } })}
      />,
    );

    expect(
      screen.queryByRole("progressbar", { name: "Captain context remaining" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/Context window exhausted/)).toBeInTheDocument();
  });
});
