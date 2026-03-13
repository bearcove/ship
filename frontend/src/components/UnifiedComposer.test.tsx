import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentSnapshot, WorktreeDiffStats } from "../generated/ship";
import { renderWithTheme } from "../test/render";
import { UnifiedComposer } from "./UnifiedComposer";

const mocks = vi.hoisted(() => ({
  diffStats: null as WorktreeDiffStats | null,
  transcription: {
    state: { tag: "idle" as const },
    result: null,
    analyser: null,
    startRecording: vi.fn(),
    stopRecording: vi.fn(),
    cancelRecording: vi.fn(),
    clearResult: vi.fn(),
  },
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    promptCaptain: async () => undefined,
    steer: async () => undefined,
    listWorktreeFiles: async () => [],
    stopAgents: async () => undefined,
  }),
}));

vi.mock("../hooks/useDocumentDrop", () => ({
  useDocumentDrop: () => false,
}));

vi.mock("../hooks/useTranscription", () => ({
  useTranscription: () => mocks.transcription,
}));

vi.mock("../hooks/useWorktreeDiffStats", () => ({
  useWorktreeDiffStats: () => mocks.diffStats,
}));

function makeAgent(role: "Captain" | "Mate", state: AgentSnapshot["state"]): AgentSnapshot {
  return {
    role: { tag: role },
    kind: { tag: role === "Captain" ? "Claude" : "Codex" },
    state,
    context_remaining_percent: 80,
    model_id: null,
    available_models: [],
    effort_config_id: null,
    effort_value_id: null,
    available_effort_values: [],
  };
}

function renderComposer(captain: AgentSnapshot, mate: AgentSnapshot) {
  return renderWithTheme(
    <UnifiedComposer
      sessionId="session-1"
      captain={captain}
      mate={mate}
      startupState={null}
      taskStatus={null}
    />,
  );
}

beforeEach(() => {
  mocks.diffStats = null;
  mocks.transcription.startRecording.mockReset();
  mocks.transcription.stopRecording.mockReset();
  mocks.transcription.cancelRecording.mockReset();
  mocks.transcription.clearResult.mockReset();
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("UnifiedComposer", () => {
  // r[verify view.agent-panel.activity]
  it("anchors captain working status on the left when diff stats are absent", () => {
    renderComposer(
      makeAgent("Captain", { tag: "Working", plan: null, activity: null }),
      makeAgent("Mate", { tag: "Idle" }),
    );

    const row = screen.getByTestId("composer-status-row");
    const workingStatus = screen.getByTestId("composer-working-status");

    expect(workingStatus).toHaveTextContent("Captain working");
    expect(row).toHaveAttribute("data-working-anchor", "left");
    expect(row.firstElementChild).toBe(workingStatus);
    expect(screen.queryByTestId("composer-diff-stats")).not.toBeInTheDocument();
  });

  // r[verify view.agent-panel.activity]
  it("keeps mate working status left-anchored when diff stats are present and pins diff stats right", () => {
    mocks.diffStats = {
      branch_name: "feature/footer-align",
      lines_added: 12n,
      lines_removed: 3n,
      files_changed: 2n,
    };

    renderComposer(
      makeAgent("Captain", { tag: "Idle" }),
      makeAgent("Mate", { tag: "Working", plan: null, activity: null }),
    );

    const row = screen.getByTestId("composer-status-row");
    const workingStatus = screen.getByTestId("composer-working-status");
    const diffStats = screen.getByTestId("composer-diff-stats");

    expect(workingStatus).toHaveTextContent("Mate working");
    expect(row).toHaveAttribute("data-working-anchor", "left");
    expect(row.firstElementChild).toBe(workingStatus);
    expect(diffStats).toHaveTextContent("feature/footer-align");
    expect(diffStats).toHaveStyle({ marginLeft: "auto" });
    expect(row.lastElementChild).toBe(diffStats);
  });
});
