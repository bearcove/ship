import { globalStyle, globalFontFace, keyframes } from "@vanilla-extract/css";

export const spinAnimation = keyframes({
  from: { transform: "rotate(0deg)" },
  to: { transform: "rotate(360deg)" },
});

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
      backgroundColor: "#ebebeb",
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
  backgroundColor: "#f0eff2",
  "@media": {
    "(prefers-color-scheme: dark)": {
      backgroundColor: "#111113",
    },
  },
});

globalStyle("pre, code, .mono", {
  fontFamily: monoFontStack,
});
