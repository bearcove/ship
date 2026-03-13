import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentSnapshot } from "../generated/ship";
import { renderWithTheme } from "../test/render";
import { UnifiedComposer } from "./UnifiedComposer";

const mocks = vi.hoisted(() => ({
  transcription: {
    state: { tag: "idle" as const },
    result: null,
    analyser: null,
    startRecording: vi.fn(),
    stopRecording: vi.fn(),
    cancelRecording: vi.fn(),
    clearResult: vi.fn(),
  },
  promptCaptain: vi.fn(async () => undefined),
  steer: vi.fn(async () => undefined),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    promptCaptain: mocks.promptCaptain,
    steer: mocks.steer,
    listWorktreeFiles: async () => [],
    stopAgents: async () => undefined,
    getWorktreeDiffStats: async () => null,
  }),
}));

vi.mock("../hooks/useDocumentDrop", () => ({
  useDocumentDrop: () => false,
}));

vi.mock("../hooks/useTranscription", () => ({
  useTranscription: () => mocks.transcription,
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

function idleComposer() {
  return renderWithTheme(
    <UnifiedComposer
      sessionId="session-1"
      captain={makeAgent("Captain", { tag: "Idle" })}
      mate={makeAgent("Mate", { tag: "Idle" })}
      startupState={null}
      taskStatus={null}
    />,
  );
}

beforeEach(() => {
  mocks.promptCaptain.mockReset();
  mocks.promptCaptain.mockResolvedValue(undefined);
  mocks.steer.mockReset();
  mocks.steer.mockResolvedValue(undefined);
  mocks.transcription.startRecording.mockReset();
  mocks.transcription.stopRecording.mockReset();
  mocks.transcription.cancelRecording.mockReset();
  mocks.transcription.clearResult.mockReset();
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("UnifiedComposer", () => {
  it("restores saved draft from localStorage on mount", () => {
    localStorage.setItem("ship.composer.draft.session-1", "my saved draft");
    idleComposer();
    const textarea = screen.getByRole("textbox", { name: /steer input/i });
    expect(textarea).toHaveValue("my saved draft");
  });

  it("clears draft from localStorage on successful submit", async () => {
    idleComposer();
    const textarea = screen.getByRole("textbox", { name: /steer input/i });
    fireEvent.change(textarea, { target: { value: "hello world" } });
    expect(localStorage.getItem("ship.composer.draft.session-1")).toBe("hello world");
    await act(async () => {
      fireEvent.keyDown(textarea, { key: "Enter" });
    });
    await act(async () => {});
    expect(localStorage.getItem("ship.composer.draft.session-1")).toBeNull();
    expect(textarea).toHaveValue("");
  });

  // r[verify ui.keys.steer-send]
  it("preserves text and shows error when submit times out", async () => {
    vi.useFakeTimers();
    mocks.promptCaptain.mockReturnValue(new Promise<undefined>(() => {}));

    idleComposer();
    const textarea = screen.getByRole("textbox", { name: /steer input/i });
    fireEvent.change(textarea, { target: { value: "will timeout" } });

    await act(async () => {
      fireEvent.keyDown(textarea, { key: "Enter" });
    });

    await act(async () => {
      vi.advanceTimersByTime(16_000);
    });

    expect(textarea).toHaveValue("will timeout");
    expect(screen.getByText(/request timed out/i)).toBeInTheDocument();

    vi.useRealTimers();
  });
});
