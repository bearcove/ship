import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentSnapshot } from "../generated/ship";
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
  setAgentModel: vi.fn(
    async (): Promise<
      { tag: "Ok" } | { tag: "Failed"; message: string } | { tag: "AgentNotSpawned" }
    > => ({
      tag: "Ok",
    }),
  ),
  setAgentEffort: vi.fn(async () => ({ tag: "Ok" })),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    setAgentModel: apiMocks.setAgentModel,
    setAgentEffort: apiMocks.setAgentEffort,
  }),
}));

function makeAgent(overrides: Partial<AgentSnapshot> = {}): AgentSnapshot {
  return {
    role: { tag: "Captain" },
    kind: { tag: "Claude" },
    state: { tag: "Working", plan: null, activity: null },
    context_remaining_percent: 82,
    preset_id: null,
    provider: null,
    model_id: "opencode/openai/gpt-5",
    available_models: [
      "opencode/openai/gpt-5",
      "opencode/anthropic/claude-sonnet-4",
      "opencode/google/gemini-2.5-pro",
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
  apiMocks.setAgentModel.mockReset();
  apiMocks.setAgentModel.mockImplementation(async () => ({ tag: "Ok" }));
  apiMocks.setAgentEffort.mockReset();
  apiMocks.setAgentEffort.mockImplementation(async () => ({ tag: "Ok" }));
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("AgentHeader", () => {
  // r[verify ui.agent-header.layout]
  // r[verify view.agent-panel.state]
  it("renders a stacked rail header with a circular context indicator and an unsplit full model id", () => {
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

    expect(within(controlRow!).getByText("opencode/openai/gpt-5")).toBeInTheDocument();
  });

  // r[verify ui.agent-header.layout]
  it("filters models from a single searchable list and selects the exact full model id", async () => {
    renderHeader(makeAgent());

    fireEvent.click(screen.getByRole("button", { name: "Select model" }));

    const searchInput = await screen.findByLabelText("Search models");
    expect(screen.getByRole("option", { name: "opencode/openai/gpt-5" })).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "opencode/anthropic/claude-sonnet-4" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "opencode/google/gemini-2.5-pro" }),
    ).toBeInTheDocument();

    fireEvent.change(searchInput, { target: { value: "claude" } });

    expect(
      screen.getByRole("option", { name: "opencode/anthropic/claude-sonnet-4" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "opencode/openai/gpt-5" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "opencode/google/gemini-2.5-pro" }),
    ).not.toBeInTheDocument();

    fireEvent.mouseDown(screen.getByRole("option", { name: "opencode/anthropic/claude-sonnet-4" }));

    await waitFor(() => {
      expect(apiMocks.setAgentModel).toHaveBeenCalledWith(
        "session-1",
        { tag: "Captain" },
        "opencode/anthropic/claude-sonnet-4",
      );
    });
  });

  // r[verify ui.agent-header.layout]
  it("renders static model text when there is only one available model id", () => {
    renderHeader(
      makeAgent({
        model_id: "gpt-5",
        available_models: ["gpt-5"],
      }),
    );

    expect(screen.getByText("gpt-5")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Select model" })).not.toBeInTheDocument();
  });

  // r[verify ui.agent-header.layout]
  it("renders dedicated effort separately while leaving the model id unsplit", () => {
    const { container } = renderHeader(
      makeAgent({
        kind: { tag: "Codex" },
        model_id: "gpt-5-codex/high",
        available_models: ["gpt-5-codex/high", "gpt-5/high"],
        effort_config_id: "reasoning.effort",
        effort_value_id: "low",
        available_effort_values: [
          { id: "low", name: "Low" },
          { id: "high", name: "High" },
        ],
      }),
    );

    const controlRows = container.querySelectorAll<HTMLElement>(`.${agentHeaderControlRow}`);
    expect(controlRows).toHaveLength(2);
    expect(within(controlRows[0]!).getByText("gpt-5-codex/high")).toBeInTheDocument();
    expect(within(controlRows[1]!).getByText("Low")).toBeInTheDocument();
  });

  // r[verify ui.agent-header.layout]
  it("preserves failed setAgentModel error handling", async () => {
    apiMocks.setAgentModel.mockResolvedValueOnce({
      tag: "Failed",
      message: "Provider unavailable",
    });

    renderHeader(makeAgent());

    fireEvent.click(screen.getByRole("button", { name: "Select model" }));
    fireEvent.mouseDown(
      await screen.findByRole("option", { name: "opencode/anthropic/claude-sonnet-4" }),
    );

    await waitFor(() => {
      expect(screen.getByText("Provider unavailable")).toBeInTheDocument();
    });
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
