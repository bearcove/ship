import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentSnapshot } from "../generated/ship";
import { renderWithTheme } from "../test/render";
import { AgentEffortPicker } from "./AgentEffortPicker";

const apiMocks = vi.hoisted(() => {
  const setAgentEffort = vi.fn(async () => ({ tag: "Ok" }));
  const getShipClient = vi.fn(async () => ({
    setAgentEffort,
  }));

  return { getShipClient, setAgentEffort };
});

vi.mock("../api/client", () => ({
  getShipClient: apiMocks.getShipClient,
}));

function makeAgent(overrides: Partial<AgentSnapshot> = {}): AgentSnapshot {
  return {
    role: { tag: "Captain" },
    kind: { tag: "Codex" },
    state: { tag: "Working", plan: null, activity: null },
    context_remaining_percent: 82,
    preset_id: null,
    provider: null,
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
  apiMocks.setAgentEffort.mockReset();
  apiMocks.setAgentEffort.mockImplementation(async () => ({ tag: "Ok" }));
  apiMocks.getShipClient.mockReset();
  apiMocks.getShipClient.mockImplementation(async () => ({
    setAgentEffort: apiMocks.setAgentEffort,
  }));
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

  it("rolls back the optimistic label and shows an error when the effort RPC throws", async () => {
    apiMocks.setAgentEffort.mockRejectedValueOnce(new Error("transport lost"));

    renderPicker(makeAgent());

    fireEvent.pointerDown(screen.getByText("Low"), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "High" }));

    await waitFor(() => {
      expect(screen.getByText("Low")).toBeInTheDocument();
    });
    expect(screen.getByText("transport lost")).toBeInTheDocument();
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
