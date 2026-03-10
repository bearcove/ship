import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithTheme } from "../test/render";
import { InlineAgentComposer } from "./InlineAgentComposer";

const apiMocks = vi.hoisted(() => ({
  promptCaptain: vi.fn(async () => undefined),
  steer: vi.fn(async () => undefined),
  cancel: vi.fn(async () => undefined),
  listWorktreeFiles: vi.fn(async () => []),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    promptCaptain: apiMocks.promptCaptain,
    steer: apiMocks.steer,
    cancel: apiMocks.cancel,
    listWorktreeFiles: apiMocks.listWorktreeFiles,
  }),
}));

beforeEach(() => {
  apiMocks.promptCaptain.mockClear();
  apiMocks.steer.mockClear();
  apiMocks.cancel.mockClear();
});

describe("InlineAgentComposer", () => {
  it("queues captain guidance during startup and sends it once the captain is no longer busy", async () => {
    const view = renderWithTheme(
      <InlineAgentComposer
        sessionId="session-1"
        role={{ tag: "Captain" }}
        agentStateTag="Working"
        startupState={{
          tag: "Running",
          stage: { tag: "GreetingCaptain" },
          message: "Greeting user",
        }}
        taskStatus={null}
      />,
    );

    const input = screen.getByLabelText("Captain steer input");
    expect(input).toBeEnabled();

    fireEvent.change(input, { target: { value: "Queue this for startup." } });
    expect(screen.getByRole("button", { name: /Queue/i })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: /Queue/i }));

    expect(apiMocks.promptCaptain).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: /Replace queue/i })).toBeInTheDocument();

    view.rerender(
      <InlineAgentComposer
        sessionId="session-1"
        role={{ tag: "Captain" }}
        agentStateTag="Idle"
        startupState={{
          tag: "Running",
          stage: { tag: "StartingMate" },
          message: "Starting mate",
        }}
        taskStatus={null}
      />,
    );

    await waitFor(() => {
      expect(apiMocks.promptCaptain).toHaveBeenCalledWith("session-1", [
        { tag: "Text", text: "Queue this for startup." },
      ]);
    });
  });

  it("sends captain guidance immediately once the captain is ready even if mate startup continues", async () => {
    renderWithTheme(
      <InlineAgentComposer
        sessionId="session-1"
        role={{ tag: "Captain" }}
        agentStateTag="Idle"
        startupState={{
          tag: "Running",
          stage: { tag: "StartingMate" },
          message: "Starting mate",
        }}
        taskStatus={null}
      />,
    );

    fireEvent.change(screen.getByLabelText("Captain steer input"), {
      target: { value: "Say hi while mate startup continues." },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/i }));

    await waitFor(() => {
      expect(apiMocks.promptCaptain).toHaveBeenCalledWith("session-1", [
        { tag: "Text", text: "Say hi while mate startup continues." },
      ]);
    });
  });

  // r[verify view.agent-panel.activity]
  it("shows activity indicator when agent is working and hides it when idle", () => {
    const view = renderWithTheme(
      <InlineAgentComposer
        sessionId="session-1"
        role={{ tag: "Captain" }}
        agentStateTag="Working"
        startupState={{ tag: "Ready" }}
        taskStatus={{ tag: "Working" }}
      />,
    );

    expect(screen.getByText("Working")).toBeInTheDocument();

    view.rerender(
      <InlineAgentComposer
        sessionId="session-1"
        role={{ tag: "Captain" }}
        agentStateTag="Idle"
        startupState={{ tag: "Ready" }}
        taskStatus={{ tag: "Working" }}
      />,
    );

    expect(screen.queryByText("Working")).not.toBeInTheDocument();
  });

  // r[verify task.cancel]
  it("sends cancel when Esc is pressed while agent is working", async () => {
    renderWithTheme(
      <InlineAgentComposer
        sessionId="session-1"
        role={{ tag: "Captain" }}
        agentStateTag="Working"
        startupState={{ tag: "Ready" }}
        taskStatus={{ tag: "Working" }}
      />,
    );

    fireEvent.keyDown(screen.getByLabelText("Captain steer input"), { key: "Escape" });

    await waitFor(() => {
      expect(apiMocks.cancel).toHaveBeenCalledWith("session-1");
    });
  });

  it("keeps mate steer editable during startup while making send state explicit", () => {
    renderWithTheme(
      <InlineAgentComposer
        sessionId="session-1"
        role={{ tag: "Mate" }}
        agentStateTag="Idle"
        startupState={{ tag: "Running", stage: { tag: "StartingMate" }, message: "Starting mate" }}
        taskStatus={null}
      />,
    );

    expect(screen.getByLabelText("Mate steer input")).toBeEnabled();
    expect(screen.getByRole("button", { name: /Send/i })).toBeDisabled();
  });
});
