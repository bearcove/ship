import { fireEvent, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionSummary } from "../generated/ship";
import { renderWithTheme } from "../test/render";
import { useGlobalKeyboard } from "./useGlobalKeyboard";

const mocks = vi.hoisted(() => ({
  transcription: {
    isRecording: vi.fn(() => false),
    startRecording: vi.fn(),
    stopAndSend: vi.fn(),
  },
  stopAgents: vi.fn(),
  archiveSession: vi.fn(),
}));

vi.mock("../context/TranscriptionContext", () => ({
  useTranscription: () => mocks.transcription,
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    stopAgents: mocks.stopAgents,
    archiveSession: mocks.archiveSession,
  }),
}));

function makeSession(): SessionSummary {
  return {
    id: "session-1",
    slug: "aaaa",
    project: "ship",
    branch_name: "main",
    title: "Voice input",
    captain: {
      role: { tag: "Captain" },
      kind: { tag: "Claude" },
      state: { tag: "Idle" },
      context_remaining_percent: null,
      preset_id: null,
      provider: null,
      model_id: null,
      available_models: [],
      effort_config_id: null,
      effort_value_id: null,
      available_effort_values: [],
    },
    mate: {
      role: { tag: "Mate" },
      kind: { tag: "Codex" },
      state: { tag: "Idle" },
      context_remaining_percent: null,
      preset_id: null,
      provider: null,
      model_id: null,
      available_models: [],
      effort_config_id: null,
      effort_value_id: null,
      available_effort_values: [],
    },
    startup_state: { tag: "Ready" },
    current_task_title: null,
    current_task_description: null,
    task_status: null,
    diff_stats: null,
    tasks_done: 0,
    tasks_total: 0,
    autonomy_mode: { tag: "HumanInTheLoop" },
    created_at: "2026-01-01T00:00:00Z",
  };
}

function Harness() {
  useGlobalKeyboard([makeSession()]);
  return <div>ready</div>;
}

beforeEach(() => {
  mocks.transcription.isRecording.mockReset();
  mocks.transcription.isRecording.mockReturnValue(false);
  mocks.transcription.startRecording.mockReset();
  mocks.transcription.stopAndSend.mockReset();
  mocks.stopAgents.mockReset();
  mocks.archiveSession.mockReset();
});

describe("useGlobalKeyboard", () => {
  it("starts voice recording with the real session id instead of the route slug", () => {
    renderWithTheme(
      <MemoryRouter initialEntries={["/sessions/aaaa"]}>
        <Routes>
          <Route path="/sessions/:sessionId" element={<Harness />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByText("ready")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: " " });

    expect(mocks.transcription.startRecording).toHaveBeenCalledWith("session-1");
  });
});
