import { screen, within } from "@testing-library/react";
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
  setAgentModel: vi.fn(async () => ({})),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    setAgentModel: apiMocks.setAgentModel,
  }),
}));

function makeAgent(overrides: Partial<AgentSnapshot> = {}): AgentSnapshot {
  return {
    role: { tag: "Captain" },
    kind: { tag: "Claude" },
    state: { tag: "Working", plan: null, activity: null },
    context_remaining_percent: 82,
    model_id: "gpt-5-codex/medium",
    available_models: ["gpt-5-codex/medium", "gpt-5-codex/low", "gpt-5/medium"],
    ...overrides,
  };
}

function renderHeader(agent: AgentSnapshot, avatarSrc?: string) {
  return renderWithTheme(<AgentHeader sessionId="session-1" agent={agent} avatarSrc={avatarSrc} />);
}

beforeEach(() => {
  apiMocks.setAgentModel.mockClear();
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("AgentHeader", () => {
  // r[verify ui.agent-header.layout]
  // r[verify view.agent-panel.state]
  it("renders a stacked rail header with a circular context indicator and split model controls", () => {
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

    expect(within(controlRow!).getByText("gpt-5-codex")).toBeInTheDocument();
    expect(within(controlRow!).getByText("medium")).toBeInTheDocument();
  });

  // r[verify ui.agent-header.layout]
  it("falls back to a single model dropdown when only unsplit model ids are available", () => {
    renderHeader(
      makeAgent({
        model_id: "gpt-5",
        available_models: ["gpt-5", "gpt-5-mini"],
      }),
    );

    expect(screen.getByText("gpt-5")).toBeInTheDocument();
    expect(screen.queryByText("medium")).not.toBeInTheDocument();
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
    expect(screen.queryByRole("button", { name: "gpt-5" })).not.toBeInTheDocument();
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
