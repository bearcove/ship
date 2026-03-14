import { keyframes, style } from "@vanilla-extract/css";
import { monoFontStack } from "./global.css";

const pulse = keyframes({
  "0%, 100%": { opacity: 1 },
  "50%": { opacity: 0.3 },
});

export const sessionCard = style({
  cursor: "pointer",
  transition: "box-shadow 0.15s",
  ":hover": {
    boxShadow: "var(--shadow-3)",
  },
});

export const idleDot = style({
  width: 8,
  height: 8,
  borderRadius: "50%",
  background: "var(--amber-9)",
  animation: `${pulse} 1.5s ease-in-out infinite`,
});

export const branchComboboxTrigger = style({
  justifyContent: "space-between",
});

export const branchComboboxList = style({
  position: "absolute",
  top: "100%",
  left: 0,
  right: 0,
  zIndex: 50,
  marginTop: 2,
  overflow: "hidden",
  border: "1px solid var(--gray-a6)",
  borderRadius: "var(--radius-3)",
  background: "var(--color-panel-solid)",
  boxShadow: "var(--shadow-4)",
});

export const branchComboboxItem = style({
  padding: "var(--space-2) var(--space-3)",
  cursor: "pointer",
  selectors: {
    '&[data-selected="true"]': {
      background: "var(--accent-a3)",
    },
    "&:hover": {
      background: "var(--gray-a3)",
    },
  },
});

const spin = keyframes({
  "0%": { transform: "rotate(0deg)" },
  "100%": { transform: "rotate(360deg)" },
});

export const statusSpinner = style({
  display: "inline-block",
  width: 14,
  height: 14,
  border: "2px solid var(--gray-a5)",
  borderTopColor: "var(--accent-9)",
  borderRadius: "50%",
  animation: `${spin} 0.8s linear infinite`,
  flexShrink: 0,
});

export const unreadCardWaitingForHuman = style({
  borderLeft: "3px solid var(--amber-9)",
});

export const unreadCardIdle = style({
  borderLeft: "3px solid var(--blue-9)",
});

export const keyboardShortcutKey = style({
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  minWidth: 20,
  padding: "0 6px",
  border: "1px solid var(--white-a5)",
  borderRadius: "var(--radius-2)",
  background: "var(--black-a2)",
  color: "inherit",
  fontSize: "12px",
  lineHeight: 1.6,
  fontFamily: monoFontStack,
});
