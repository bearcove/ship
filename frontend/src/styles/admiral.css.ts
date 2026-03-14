import { style } from "@vanilla-extract/css";

export const activityBubble = style({
  padding: "var(--space-3) var(--space-4)",
  borderRadius: "var(--radius-3)",
  background: "var(--color-panel-solid)",
  border: "1px solid var(--gray-a4)",
  maxWidth: 600,
});

export const activityTimeline = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-3)",
  padding: "var(--space-4)",
  overflowY: "auto",
  flex: 1,
});
