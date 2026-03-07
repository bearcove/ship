import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithTheme } from "../test/render";
import { InlineAgentComposer } from "./InlineAgentComposer";

const apiMocks = vi.hoisted(() => ({
  promptCaptain: vi.fn(async () => undefined),
  steer: vi.fn(async () => undefined),
}));

vi.mock("../api/client", () => ({
  getShipClient: async () => ({
    promptCaptain: apiMocks.promptCaptain,
    steer: apiMocks.steer,
  }),
}));

beforeEach(() => {
  apiMocks.promptCaptain.mockClear();
  apiMocks.steer.mockClear();
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
    expect(screen.getByText("Queued")).toBeInTheDocument();

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
      expect(apiMocks.promptCaptain).toHaveBeenCalledWith("session-1", "Queue this for startup.");
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
      expect(apiMocks.promptCaptain).toHaveBeenCalledWith(
        "session-1",
        "Say hi while mate startup continues.",
      );
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
    expect(
      screen.getByText(
        "You can draft mate steer now. Sending unlocks after startup and task setup.",
      ),
    ).toBeInTheDocument();
  });
});
