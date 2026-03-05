import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { Theme } from "@radix-ui/themes";
import "@radix-ui/themes/styles.css";
import "./styles/global.css.ts";
import { App } from "./App";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <Theme
        appearance="dark"
        accentColor="iris"
        grayColor="slate"
        radius="medium"
        scaling="100%"
      >
        <App />
      </Theme>
    </BrowserRouter>
  </StrictMode>,
);
