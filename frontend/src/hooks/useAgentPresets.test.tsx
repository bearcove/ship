import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentPreset } from "../generated/ship";
import { renderWithTheme } from "../test/render";
import { resetAgentPresetsForTest, useAgentPresets } from "./useAgentPresets";

const apiMocks = vi.hoisted(() => {
  const reconnectListeners = new Set<() => void>();

  return {
    listAgentPresets: vi.fn<() => Promise<AgentPreset[]>>(async () => []),
    onClientReady: vi.fn((listener: () => void) => {
      reconnectListeners.add(listener);
      return () => {
        reconnectListeners.delete(listener);
      };
    }),
    emitClientReady: () => {
      for (const listener of reconnectListeners) {
        listener();
      }
    },
    resetClientReady: () => {
      reconnectListeners.clear();
    },
  };
});

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    listAgentPresets: apiMocks.listAgentPresets,
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
    logo: null,
  },
];

function PresetStatus() {
  const { presets, error, loading } = useAgentPresets();

  return (
    <div>
      <div>loading:{loading ? "yes" : "no"}</div>
      <div>error:{error ?? "none"}</div>
      <div>count:{presets.length}</div>
    </div>
  );
}

beforeEach(() => {
  resetAgentPresetsForTest();
  apiMocks.resetClientReady();
  apiMocks.listAgentPresets.mockReset();
  apiMocks.onClientReady.mockClear();
});

describe("useAgentPresets", () => {
  it("recovers from an initial load failure after the client reconnect callback fires", async () => {
    apiMocks.listAgentPresets
      .mockRejectedValueOnce(new Error("startup failed"))
      .mockResolvedValueOnce(configuredPresets);

    renderWithTheme(<PresetStatus />);

    expect(await screen.findByText("error:startup failed")).toBeInTheDocument();
    expect(screen.getByText("count:0")).toBeInTheDocument();

    apiMocks.emitClientReady();

    await waitFor(() => {
      expect(screen.getByText("error:none")).toBeInTheDocument();
    });
    expect(screen.getByText("count:1")).toBeInTheDocument();
    expect(apiMocks.listAgentPresets).toHaveBeenCalledTimes(2);
  });
});
