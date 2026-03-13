import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentSnapshot } from "../generated/ship";
import { renderWithTheme } from "../test/render";
import { AgentEffortPicker } from "./AgentEffortPicker";

const apiMocks = vi.hoisted(() => ({
  setAgentEffort: vi.fn(async () => ({ tag: "Ok" })),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    setAgentEffort: apiMocks.setAgentEffort,
  }),
}));

function makeAgent(overrides: Partial<AgentSnapshot> = {}): AgentSnapshot {
  return {
    role: { tag: "Captain" },
    kind: { tag: "Codex" },
    state: { tag: "Working", plan: null, activity: null },
    context_remaining_percent: 82,
    model_id: "gpt-5-codex/high",
    available_models: ["gpt-5-codex/high", "gpt-5/high"],
    effort_config_id: "reasoning.effort",
    effort_value_id: "low",
    available_effort_values: [
      { id: "low", name: "Low" },
      { id: "high", name: "High" },
    ],
    ...overrides,
  };
}

function renderPicker(agent: AgentSnapshot) {
  return renderWithTheme(<AgentEffortPicker sessionId="session-1" agent={agent} />);
}

beforeEach(() => {
  apiMocks.setAgentEffort.mockClear();
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("AgentEffortPicker", () => {
  it("calls setAgentEffort with the selected config and value ids and updates the visible label", async () => {
    renderPicker(makeAgent());

    fireEvent.pointerDown(screen.getByText("Low"), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "High" }));

    await waitFor(() => {
      expect(apiMocks.setAgentEffort).toHaveBeenCalledWith(
        "session-1",
        { tag: "Captain" },
        "reasoning.effort",
        "high",
      );
    });

    expect(screen.getByText("High")).toBeInTheDocument();
    expect(screen.queryByText("Low")).not.toBeInTheDocument();
  });

  it("updates the displayed effort when the agent snapshot changes", () => {
    const { rerender } = renderPicker(makeAgent());

    expect(screen.getByText("Low")).toBeInTheDocument();

    rerender(
      <AgentEffortPicker
        sessionId="session-1"
        agent={makeAgent({
          effort_value_id: "high",
        })}
      />,
    );

    expect(screen.getByText("High")).toBeInTheDocument();
    expect(screen.queryByText("Low")).not.toBeInTheDocument();
  });
});
