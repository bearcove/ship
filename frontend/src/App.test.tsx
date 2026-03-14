import { screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { SoundProvider } from "./context/SoundContext";
import { renderWithTheme } from "./test/render";

vi.mock("./api/client", () => ({
  getConnectionState: () => "connected",
  onConnectionStateChanged: () => () => {},
  onClientReady: () => () => {},
  useClientLogs: () => [],
}));

vi.mock("./components/ConnectionBanner", () => ({
  ConnectionBanner: () => null,
}));

vi.mock("./components/NotificationPrompt", () => ({
  NotificationPrompt: () => null,
}));

vi.mock("./hooks/useSessionList", () => ({
  useSessionList: () => [],
}));

vi.mock("./hooks/useProjects", () => ({
  useProjects: () => [],
}));

vi.mock("./hooks/useAgentDiscovery", () => ({
  useAgentDiscovery: () => ({ claude: false, codex: false, opencode: false }),
}));

vi.mock("./components/NewSessionDialog", () => ({
  NewSessionDialog: () => null,
}));

vi.mock("./pages/SessionViewPage", () => ({
  SessionViewPage: () => <div>Session view page</div>,
}));

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("App shell navigation", () => {
  // r[verify ui.layout.shell]
  it("redirects / to /sessions/admiral", () => {
    renderWithTheme(
      <MemoryRouter initialEntries={["/"]}>
        <SoundProvider>
          <App />
        </SoundProvider>
      </MemoryRouter>,
    );

    // The / route redirects to /sessions/admiral, so SessionViewPage renders
    expect(screen.getByText("Session view page")).toBeInTheDocument();
  });
});
