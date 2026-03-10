import { style } from "@vanilla-extract/css";

export const sidebarRoot = style({
  width: 210,
  flexShrink: 0,
  display: "flex",
  flexDirection: "column",
  borderRight: "1px solid var(--gray-a5)",
  overflow: "hidden",
  transition: "width 0.15s ease",
  "@media": {
    "(max-width: 1023px)": {
      display: "none",
    },
  },
  selectors: {
    '&[data-collapsed="true"]': {
      width: 44,
    },
  },
});

export const sidebarScrollArea = style({
  flex: 1,
  overflowY: "auto",
  overflowX: "hidden",
});

export const sidebarTab = style({
  display: "block",
  padding: "var(--space-2) var(--space-3)",
  textDecoration: "none",
  color: "inherit",
  borderBottom: "1px solid var(--gray-a3)",
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

export const sidebarTabProject = style({
  fontSize: "var(--font-size-1)",
  fontWeight: "var(--font-weight-medium)",
  color: "var(--gray-11)",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const sidebarTabDesc = style({
  fontSize: "var(--font-size-1)",
  color: "var(--gray-12)",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  marginTop: 2,
});

export const sidebarStatusRow = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-1)",
  marginTop: 2,
});

export const sidebarStatusDot = style({
  width: 6,
  height: 6,
  borderRadius: "50%",
  flexShrink: 0,
});

export const sidebarFooter = style({
  flexShrink: 0,
  padding: "var(--space-2) var(--space-3)",
  borderTop: "1px solid var(--gray-a5)",
});
