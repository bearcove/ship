import { act, fireEvent, screen } from "@testing-library/react";
import { createRef, type Ref } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentSnapshot } from "../generated/ship";
import { createBlockStore } from "../state/blockStore";
import { renderWithTheme } from "../test/render";
import { UnifiedComposer, type UnifiedComposerHandle } from "./UnifiedComposer";

const mocks = vi.hoisted(() => ({
  transcription: {
    state: { tag: "idle" } as { tag: "idle" } | { tag: "recording"; elapsed: number },
    result: null as { text: string; segments: unknown[] } | null,
    analyser: null,
    targetSessionId: null as string | null,
    sendAfterTranscription: false,
    startRecording: vi.fn(),
    stopRecording: vi.fn(),
    stopAndSend: vi.fn(),
    cancelRecording: vi.fn(),
    clearResult: vi.fn(),
    clearVoiceSubmit: vi.fn(),
    dismissError: vi.fn(),
    voiceMode: false,
    voiceSubmitText: null as string | null,
    isRecording: vi.fn(() => false),
  },
  playback: null as {
    state: "idle" | "loading" | "playing";
    analyser: AnalyserNode | null;
    stop: () => void;
  } | null,
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

vi.mock("../context/TranscriptionContext", () => ({
  useTranscription: () => mocks.transcription,
}));

vi.mock("../context/PlaybackContext", () => ({
  usePlayback: () => mocks.playback,
}));

function makeAgent(role: "Captain" | "Mate", state: AgentSnapshot["state"]): AgentSnapshot {
  return {
    role: { tag: role },
    kind: { tag: role === "Captain" ? "Claude" : "Codex" },
    state,
    context_remaining_percent: 80,
    preset_id: null,
    provider: null,
    model_id: null,
    available_models: [],
    effort_config_id: null,
    effort_value_id: null,
    available_effort_values: [],
  };
}

function renderComposer(sessionId = "session-1", ref?: Ref<UnifiedComposerHandle>) {
  return renderWithTheme(
    <UnifiedComposer
      ref={ref}
      sessionId={sessionId}
      captain={makeAgent("Captain", { tag: "Idle" })}
      mate={makeAgent("Mate", { tag: "Idle" })}
      startupState={null}
      taskStatus={null}
      captainBlocks={createBlockStore()}
    />,
  );
}

beforeEach(() => {
  localStorage.clear();
  mocks.transcription.state = { tag: "idle" };
  mocks.transcription.result = null;
  mocks.transcription.analyser = null;
  mocks.transcription.targetSessionId = null;
  mocks.transcription.sendAfterTranscription = false;
  mocks.playback = null;
  mocks.promptCaptain.mockReset();
  mocks.promptCaptain.mockResolvedValue(undefined);
  mocks.steer.mockReset();
  mocks.steer.mockResolvedValue(undefined);
  mocks.transcription.startRecording.mockReset();
  mocks.transcription.stopRecording.mockReset();
  mocks.transcription.stopAndSend.mockReset();
  mocks.transcription.cancelRecording.mockReset();
  mocks.transcription.clearResult.mockReset();
  mocks.transcription.isRecording.mockReset();
  mocks.transcription.isRecording.mockReturnValue(false);
  mocks.transcription.targetSessionId = null;
  mocks.transcription.sendAfterTranscription = false;
});

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("UnifiedComposer", () => {
  it("restores saved draft from localStorage on mount", () => {
    localStorage.setItem("ship.composer.draft.session-1", "my saved draft");
    renderComposer();
    const textarea = screen.getByRole("textbox", { name: /steer input/i });
    expect(textarea).toHaveValue("my saved draft");
  });

  it("clears draft from localStorage on successful submit", async () => {
    renderComposer();
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

  it("insertQuote via ref inserts blockquoted text into the textarea", () => {
    const ref = createRef<UnifiedComposerHandle>();
    renderComposer("session-1", ref);

    act(() => ref.current!.insertQuote("hello\nworld"));

    const textarea = screen.getByRole("textbox", { name: /steer input/i });
    expect(textarea).toHaveValue("> hello\n> world\n\n");
  });

  it("keeps the imperative handle wired to the current composer after rerenders", () => {
    const ref = createRef<UnifiedComposerHandle>();
    const view = renderComposer("session-1", ref);

    act(() => ref.current!.setDragOver(true));
    expect(screen.getByTestId("composer-drop-indicator")).toBeInTheDocument();

    view.rerender(
      <UnifiedComposer
        ref={ref}
        sessionId="session-2"
        captain={makeAgent("Captain", { tag: "Idle" })}
        mate={makeAgent("Mate", { tag: "Idle" })}
        startupState={null}
        taskStatus={null}
        captainBlocks={createBlockStore()}
      />,
    );

    const textarea = screen.getByRole("textbox", { name: /steer input/i });
    act(() => ref.current!.setDragOver(false));
    expect(screen.queryByTestId("composer-drop-indicator")).toBeNull();

    act(() => ref.current!.focusComposer());
    expect(textarea).toHaveFocus();

    act(() => ref.current!.insertQuote("fresh quote"));
    expect(textarea).toHaveValue("> fresh quote\n\n");
  });

  it("insertQuote prepends quote before existing text", () => {
    const ref = createRef<UnifiedComposerHandle>();
    renderComposer("session-1", ref);

    const textarea = screen.getByRole("textbox", { name: /steer input/i });
    fireEvent.change(textarea, { target: { value: "existing reply" } });

    act(() => ref.current!.insertQuote("quoted text"));

    expect(textarea).toHaveValue("> quoted text\n\nexisting reply");
  });

  it("drops a cancelled recording prefix before applying the next transcription result", () => {
    const view = renderComposer();
    const textarea = screen.getByRole("textbox", { name: /steer input/i });

    fireEvent.change(textarea, { target: { value: "first draft" } });

    mocks.transcription.targetSessionId = "session-1";
    mocks.transcription.state = { tag: "recording", elapsed: 0 };
    view.rerender(
      <UnifiedComposer
        sessionId="session-1"
        captain={makeAgent("Captain", { tag: "Idle" })}
        mate={makeAgent("Mate", { tag: "Idle" })}
        startupState={null}
        taskStatus={null}
        captainBlocks={createBlockStore()}
      />,
    );

    mocks.transcription.targetSessionId = null;
    mocks.transcription.state = { tag: "idle" };
    view.rerender(
      <UnifiedComposer
        sessionId="session-1"
        captain={makeAgent("Captain", { tag: "Idle" })}
        mate={makeAgent("Mate", { tag: "Idle" })}
        startupState={null}
        taskStatus={null}
        captainBlocks={createBlockStore()}
      />,
    );

    fireEvent.change(textarea, { target: { value: "second draft" } });

    mocks.transcription.targetSessionId = "session-1";
    mocks.transcription.state = { tag: "recording", elapsed: 0 };
    view.rerender(
      <UnifiedComposer
        sessionId="session-1"
        captain={makeAgent("Captain", { tag: "Idle" })}
        mate={makeAgent("Mate", { tag: "Idle" })}
        startupState={null}
        taskStatus={null}
        captainBlocks={createBlockStore()}
      />,
    );

    mocks.transcription.result = { text: "spoken words", segments: [] };
    mocks.transcription.state = { tag: "idle" };
    view.rerender(
      <UnifiedComposer
        sessionId="session-1"
        captain={makeAgent("Captain", { tag: "Idle" })}
        mate={makeAgent("Mate", { tag: "Idle" })}
        startupState={null}
        taskStatus={null}
        captainBlocks={createBlockStore()}
      />,
    );

    expect(textarea).toHaveValue("second draft spoken words");
    expect(mocks.transcription.clearResult).toHaveBeenCalledTimes(1);
  });

  // r[verify ui.keys.steer-send]
  it("preserves text and shows error when submit times out", async () => {
    vi.useFakeTimers();
    mocks.promptCaptain.mockReturnValue(new Promise<undefined>(() => {}));

    renderComposer();
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
