import { fireEvent, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { SoundProvider } from "./context/SoundContext";
import { renderWithTheme } from "./test/render";

vi.mock("./components/ConnectionBanner", () => ({
  ConnectionBanner: () => null,
}));

vi.mock("./components/NotificationPrompt", () => ({
  NotificationPrompt: () => null,
}));

vi.mock("./pages/SessionListPage", () => ({
  SessionListPage: () => <div>Session list page</div>,
}));

vi.mock("./pages/SessionViewPage", () => ({
  SessionViewPage: () => <div>Session view page</div>,
}));

// r[verify frontend.test.vitest]
// r[verify frontend.test.rtl]
describe("App shell navigation", () => {
  // r[verify ui.layout.shell]
  it("uses the Ship title as a home link back to the session list", async () => {
    renderWithTheme(
      <MemoryRouter initialEntries={["/sessions/session-1"]}>
        <SoundProvider>
          <App />
        </SoundProvider>
      </MemoryRouter>,
    );

    expect(screen.getByText("Session view page")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("link", { name: "Ship" }));

    expect(await screen.findByText("Session list page")).toBeInTheDocument();
  });
});
