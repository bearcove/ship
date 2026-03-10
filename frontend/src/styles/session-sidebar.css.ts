import { style } from "@vanilla-extract/css";

export const sidebarRoot = style({
  width: 220,
  flexShrink: 0,
  display: "flex",
  flexDirection: "column",
  borderRight: "1px solid var(--gray-a5)",
  overflow: "hidden",
  "@media": {
    "(max-width: 500px)": {
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
    "(max-width: 500px)": {
      display: "block",
      position: "fixed",
      inset: 0,
      zIndex: 199,
      background: "rgba(0,0,0,0.4)",
    },
  },
});

export const sidebarHeader = style({
  padding: "var(--space-2) var(--space-3)",
  borderBottom: "1px solid var(--gray-a5)",
  flexShrink: 0,
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-1)",
});

export const agentKindRow = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
});

export const sidebarScrollArea = style({
  flex: 1,
  overflowY: "auto",
  overflowX: "hidden",
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
  fontSize: "var(--font-size-1)",
  fontWeight: "var(--font-weight-medium)",
  color: "var(--gray-11)",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const projectActions = style({
  display: "flex",
  alignItems: "center",
  opacity: 0,
  transition: "opacity 0.1s",
  selectors: {
    [`${projectRow}:hover &`]: {
      opacity: 1,
    },
  },
});

export const sessionRow = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  paddingLeft: "calc(var(--space-3) + 18px)",
  paddingRight: "var(--space-3)",
  paddingTop: "5px",
  paddingBottom: "5px",
  textDecoration: "none",
  color: "inherit",
  selectors: {
    '&[data-active="true"]': {
      background: "var(--accent-a3)",
    },
    "&:hover": {
      background: "var(--gray-a3)",
    },
    '&[data-active="true"]:hover': {
      background: "var(--accent-a4)",
    },
  },
});

export const sessionRowTitle = style({
  flex: 1,
  fontSize: "var(--font-size-1)",
  color: "var(--gray-12)",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const sessionRowEmpty = style({
  paddingLeft: "calc(var(--space-3) + 18px)",
  paddingRight: "var(--space-3)",
  paddingTop: "3px",
  paddingBottom: "5px",
  fontSize: "var(--font-size-1)",
  color: "var(--gray-9)",
  fontStyle: "italic",
});

export const sidebarFooter = style({
  flexShrink: 0,
  padding: "var(--space-2) var(--space-3)",
  borderTop: "1px solid var(--gray-a5)",
});

export const sidebarStatusDot = style({
  width: 6,
  height: 6,
  borderRadius: "50%",
  flexShrink: 0,
});
