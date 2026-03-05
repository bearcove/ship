import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { Theme } from "@radix-ui/themes";
import "@radix-ui/themes/styles.css";
import "./styles/global.css.ts";
import { App } from "./App";
import { SoundProvider } from "./context/SoundContext";
import { ScenarioProvider } from "./context/ScenarioContext";

// r[frontend.react]
// r[frontend.routing]
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      {/* r[frontend.components.radix-theme] r[ui.theme.config] r[ui.theme.dark-only] */}
      <Theme appearance="dark" accentColor="iris" grayColor="slate" radius="medium" scaling="100%">
        <SoundProvider>
          <ScenarioProvider>
            <App />
          </ScenarioProvider>
        </SoundProvider>
      </Theme>
    </BrowserRouter>
  </StrictMode>,
);
