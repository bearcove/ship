import { globalStyle } from "@vanilla-extract/css";

export const monoFontStack =
  "'SF Mono', 'Cascadia Code', 'Fira Code', 'JetBrains Mono', 'Menlo', 'Consolas', monospace";

export const sansFontStack =
  "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif";

globalStyle("html, body, #root", {
  height: "100%",
  margin: 0,
  padding: 0,
});

globalStyle("pre, code, .mono", {
  fontFamily: monoFontStack,
});
