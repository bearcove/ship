import { globalStyle, globalFontFace } from "@vanilla-extract/css";

globalFontFace("Maple Mono NF", {
  src: "url('/fonts/MapleMono-NF-Regular.woff2') format('woff2')",
  fontWeight: "400",
  fontStyle: "normal",
});

globalFontFace("Maple Mono NF", {
  src: "url('/fonts/MapleMono-NF-Italic.woff2') format('woff2')",
  fontWeight: "400",
  fontStyle: "italic",
});

globalFontFace("Maple Mono NF", {
  src: "url('/fonts/MapleMono-NF-Bold.woff2') format('woff2')",
  fontWeight: "700",
  fontStyle: "normal",
});

globalFontFace("Maple Mono NF", {
  src: "url('/fonts/MapleMono-NF-BoldItalic.woff2') format('woff2')",
  fontWeight: "700",
  fontStyle: "italic",
});

export const monoFontStack =
  "'Maple Mono NF', 'SF Mono', 'Cascadia Code', 'Fira Code', 'JetBrains Mono', 'Menlo', 'Consolas', monospace";

export const sansFontStack =
  "'Cabin', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif";

globalStyle("html, body", {
  height: "100dvh",
  margin: 0,
  padding: 0,
  overflow: "hidden",
  overscrollBehavior: "none",
});

globalStyle("#root", {
  height: "100%",
  margin: 0,
  padding: 0,
  overflow: "hidden",
});

// Fill the browser chrome (iOS status bar, overscroll area) with the app background.
globalStyle("html", {
  backgroundColor: "#111113",
  "@media": {
    "(prefers-color-scheme: light)": {
      backgroundColor: "#ffffff",
    },
  },
});

globalStyle(".radix-themes", {
  vars: {
    "--default-font-family": sansFontStack,
    "--heading-font-family": sansFontStack,
    "--code-font-family": monoFontStack,
    "--strong-font-family": sansFontStack,
  },
});

globalStyle("pre, code, .mono", {
  fontFamily: monoFontStack,
});
