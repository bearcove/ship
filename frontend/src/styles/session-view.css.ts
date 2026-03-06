import { globalStyle, style } from "@vanilla-extract/css";

export const sessionViewRoot = style({
  display: "flex",
  flexDirection: "column",
  height: "100%",
  overflow: "hidden",
});

export const sessionTopBar = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-3)",
  padding: "var(--space-2) var(--space-4)",
  borderBottom: "1px solid var(--gray-a5)",
  flexShrink: 0,
});

export const sessionBranch = style({
  fontFamily: "monospace",
  fontSize: "var(--font-size-2)",
  color: "var(--gray-11)",
  marginRight: "auto",
});

export const panelGrid = style({
  display: "grid",
  gridTemplateColumns: "1fr 1fr",
  flex: 1,
  overflow: "hidden",
  "@media": {
    "(max-width: 1023px)": {
      gridTemplateColumns: "1fr",
    },
  },
});

export const panelColumn = style({
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
  borderRight: "1px solid var(--gray-a5)",
  selectors: {
    "&:last-child": {
      borderRight: "none",
    },
  },
});

export const mobileTabs = style({
  display: "none",
  "@media": {
    "(max-width: 1023px)": {
      display: "flex",
    },
  },
});

export const desktopGrid = style({
  display: "grid",
  gridTemplateColumns: "1fr 1fr",
  flex: 1,
  overflow: "hidden",
  "@media": {
    "(max-width: 1023px)": {
      display: "none",
    },
  },
});

export const mobilePanel = style({
  display: "none",
  flex: 1,
  overflow: "hidden",
  "@media": {
    "(max-width: 1023px)": {
      display: "flex",
      flexDirection: "column",
    },
  },
});

export const agentPanelRoot = style({
  display: "flex",
  flexDirection: "column",
  height: "100%",
  overflow: "hidden",
});

export const agentPanelScrollArea = style({
  flex: 1,
  overflowY: "auto",
  display: "flex",
  flexDirection: "column",
});

export const agentHeader = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
  padding: "var(--space-3) var(--space-3) var(--space-2)",
  borderBottom: "1px solid var(--gray-a4)",
  flexShrink: 0,
});

export const agentHeaderRow = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
});

export const stickyPlan = style({
  position: "sticky",
  top: 0,
  zIndex: 1,
  flexShrink: 0,
  padding: "var(--space-2) var(--space-3)",
  borderBottom: "1px solid var(--gray-a4)",
  background: "var(--gray-1)",
});

export const eventStream = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
  padding: "var(--space-3)",
});

export const taskBar = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-3)",
  padding: "var(--space-2) var(--space-4)",
  borderTop: "1px solid var(--gray-a5)",
  flexShrink: 0,
});

export const taskDescription = style({
  flex: 1,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  minWidth: 0,
});

export const toolCallBlock = style({
  borderRadius: "var(--radius-2)",
  border: "1px solid var(--gray-a4)",
  overflow: "hidden",
  fontSize: "var(--font-size-1)",
});

export const toolCallHeader = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  padding: "var(--space-1) var(--space-2)",
  cursor: "pointer",
  userSelect: "none",
  ":hover": {
    background: "var(--gray-a2)",
  },
});

export const toolCallBody = style({
  borderTop: "1px solid var(--gray-a4)",
  padding: "var(--space-2)",
  fontFamily: "monospace",
  fontSize: "var(--font-size-1)",
  whiteSpace: "pre-wrap",
  overflowX: "auto",
  maxHeight: "20rem",
  overflowY: "auto",
});

export const diffAdd = style({
  color: "var(--green-11)",
  background: "var(--green-a3)",
  display: "block",
});

export const diffRemove = style({
  color: "var(--red-11)",
  background: "var(--red-a3)",
  display: "block",
});

export const diffContext = style({
  display: "block",
  color: "var(--gray-11)",
});

export const textBlockRoot = style({
  color: "var(--gray-12)",
  fontSize: "var(--font-size-2)",
  lineHeight: "var(--line-height-3)",
});

globalStyle(`${textBlockRoot} :where(p, ul, ol, blockquote)`, {
  margin: "0 0 var(--space-2)",
});

globalStyle(
  `${textBlockRoot} :where(p:last-child, ul:last-child, ol:last-child, blockquote:last-child)`,
  {
    marginBottom: "0",
  },
);

globalStyle(`${textBlockRoot} :where(ul, ol)`, {
  paddingLeft: "var(--space-5)",
});

globalStyle(`${textBlockRoot} :where(pre)`, {
  margin: 0,
});

export const textBlockCodeBlock = style({
  overflow: "hidden",
  borderRadius: "var(--radius-2)",
});

globalStyle(`${textBlockCodeBlock} pre`, {
  margin: 0,
  overflowX: "auto",
});

export const textBlockCodeFallback = style({
  margin: 0,
  padding: "var(--space-2)",
  borderRadius: "var(--radius-2)",
  background: "var(--gray-a2)",
  fontFamily: "monospace",
  fontSize: "var(--font-size-1)",
  whiteSpace: "pre-wrap",
  overflowX: "auto",
});

export const steerReviewCard = style({
  margin: "var(--space-2) var(--space-4)",
  flexShrink: 0,
});

export const permissionCard = style({
  background: "var(--amber-a3)",
  border: "1px solid var(--amber-a6)",
  borderRadius: "var(--radius-3)",
  padding: "var(--space-3)",
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
});

export const idleBanner = style({
  flexShrink: 0,
  borderBottom: "1px solid var(--amber-a6)",
});
