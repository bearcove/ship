import { keyframes, style } from "@vanilla-extract/css";

export const sidebarRoot = style({
  width: "100%",
  height: "100%",
  flexShrink: 0,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
  "@media": {
    "(max-width: 700px)": {
      position: "fixed",
      left: 0,
      top: 0,
      bottom: 0,
      width: 260,
      zIndex: 200,
      background: "var(--color-background)",
      transform: "translateX(-100%)",
      transition: "transform 0.2s ease",
      borderRight: "1px solid var(--gray-a6)",
      boxShadow: "4px 0 16px rgba(0,0,0,0.2)",
    },
  },
  selectors: {
    '&[data-open="true"]': {
      transform: "translateX(0)",
    },
  },
});

export const sidebarBackdrop = style({
  display: "none",
  "@media": {
    "(max-width: 700px)": {
      display: "block",
      position: "fixed",
      inset: 0,
      zIndex: 199,
      background: "rgba(0,0,0,0.4)",
    },
  },
});

export const sidebarHomeLink = style({
  display: "none",
  "@media": {
    "(min-width: 701px)": {
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      padding: "var(--space-3) var(--space-3) var(--space-1)",
      flexShrink: 0,
    },
  },
});

export const sidebarLogoFlip = style({
  cursor: "pointer",
  display: "flex",
  justifyContent: "center",
  transition: "transform 0.6s ease",
  selectors: {
    '&[data-flipping="true"]': {
      transform: "rotateY(360deg)",
    },
  },
});

export const sidebarScrollArea = style({
  flex: 1,
  overflowY: "auto",
  overflowX: "hidden",
  paddingTop: "var(--space-3)",
  paddingBottom: "var(--space-5)",
});

export const projectRow = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-1)",
  padding: "var(--space-2) var(--space-2) var(--space-2) var(--space-3)",
  cursor: "pointer",
  userSelect: "none",
  selectors: {
    "&:hover": {
      background: "var(--gray-a2)",
    },
  },
});

export const projectName = style({
  flex: 1,
  fontSize: "var(--font-size-2)",
  fontWeight: "var(--font-weight-medium)",
  color: "var(--gray-11)",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const projectActions = style({
  display: "flex",
  alignItems: "center",
});

export const sessionRow = style({
  position: "relative",
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  fontSize: "var(--font-size-2)",
  paddingLeft: "var(--space-3)",
  paddingRight: "var(--space-3)",
  paddingTop: "var(--space-2)",
  paddingBottom: "var(--space-2)",
  margin: "var(--space-1) var(--space-2)",
  borderRadius: "var(--radius-3)",
  textDecoration: "none",
  color: "inherit",
  selectors: {
    '&[data-active="true"]': {
      background: "var(--color-background)",
      outline: "1px solid var(--gray-a4)",
      boxShadow: "0 1px 3px rgba(0,0,0,0.06)",
    },
    "&:hover": {
      background: "var(--gray-a2)",
    },
    '&[data-active="true"]:hover': {
      background: "var(--color-background)",
    },
  },
});

export const sessionRowTitle = style({
  flex: 1,
  fontSize: "var(--font-size-3)",
  lineHeight: "var(--line-height-3)",
  fontWeight: "var(--font-weight-medium)",
  color: "var(--gray-12)",
});

export const sessionRowEmpty = style({
  fontSize: "var(--font-size-2)",
  paddingLeft: "var(--space-3)",
  paddingRight: "var(--space-3)",
  paddingTop: "3px",
  paddingBottom: "5px",
  color: "var(--gray-9)",
});

export const sidebarStatusDot = style({
  width: 6,
  height: 6,
  borderRadius: "50%",
  flexShrink: 0,
});

export const sessionRowArchiveBtn = style({
  flexShrink: 0,
});

export const sessionGroupLabel = style({
  fontSize: "var(--font-size-1)",
  textTransform: "uppercase",
  letterSpacing: "0.05em",
  color: "var(--gray-9)",
  paddingTop: "var(--space-3)",
  paddingLeft: "var(--space-3)",
  paddingRight: "var(--space-3)",
  paddingBottom: "var(--space-1)",
});

const sidebarSpin = keyframes({
  "0%": { transform: "rotate(0deg)" },
  "100%": { transform: "rotate(360deg)" },
});

export const sidebarSpinner = style({
  display: "inline-block",
  width: 10,
  height: 10,
  border: "1.5px solid var(--gray-a5)",
  borderTopColor: "var(--gray-9)",
  borderRadius: "50%",
  animation: `${sidebarSpin} 3s linear infinite`,
  flexShrink: 0,
});
