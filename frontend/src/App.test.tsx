import { fireEvent, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { SoundProvider } from "./context/SoundContext";
import { renderWithTheme } from "./test/render";

vi.mock("./api/client", () => ({
  getConnectionState: () => "connected",
  onConnectionStateChanged: () => () => {},
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

vi.mock("./pages/SessionListPage", () => ({
  SessionListPage: () => <div>Session list page</div>,
  NewSessionDialog: () => null,
  AddProjectDialog: () => null,
}));

vi.mock("./pages/SessionViewPage", () => ({
  SessionViewPage: () => <div>Session view page</div>,
}));

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("App shell navigation", () => {
  // r[verify ui.layout.shell]
  it("uses the Ship title as a home link on the session list shell", async () => {
    renderWithTheme(
      <MemoryRouter initialEntries={["/"]}>
        <SoundProvider>
          <App />
        </SoundProvider>
      </MemoryRouter>,
    );

    expect(screen.getByText("Session list page")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("link", { name: "Ship" }));

    expect(await screen.findByText("Session list page")).toBeInTheDocument();
  });
});
