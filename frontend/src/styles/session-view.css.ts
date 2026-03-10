import { globalStyle, keyframes, style } from "@vanilla-extract/css";
import { monoFontStack } from "./global.css";

export const sessionViewRoot = style({
  display: "flex",
  flexDirection: "column",
  height: "100%",
  overflow: "hidden",
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

export const mobileNavBar = style({
  display: "none",
  flexShrink: 0,
  borderBottom: "1px solid var(--gray-a5)",
  "@media": {
    "(max-width: 500px)": {
      display: "block",
    },
  },
});

export const desktopGrid = style({
  display: "grid",
  gridTemplateColumns: "1fr 1fr",
  flex: 1,
  overflow: "hidden",
  "@media": {
    "(max-width: 500px)": {
      display: "none",
    },
  },
});

export const mobileStack = style({
  display: "none",
  "@media": {
    "(max-width: 500px)": {
      display: "flex",
      flex: 1,
      flexDirection: "column",
      overflow: "hidden",
    },
  },
});

export const mobileStackPanel = style({
  flex: 1,
  minHeight: 0,
  overflow: "hidden",
  display: "flex",
  flexDirection: "column",
  selectors: {
    "&:first-child": {
      borderBottom: "1px solid var(--gray-a5)",
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
  minHeight: 0,
});

export const feedMessageRow = style({
  display: "flex",
  width: "100%",
});

export const feedMessageCard = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-1)",
  padding: "var(--space-2)",
  borderRadius: "var(--radius-3)",
  border: "1px solid var(--gray-a4)",
  background: "var(--gray-a2)",
  width: "80%",
});

export const feedMessageCardAgent = style({
  marginRight: "auto",
});

export const feedMessageCardThought = style({
  marginRight: "auto",
  background: "var(--gray-a1)",
  borderStyle: "dashed",
  color: "var(--gray-11)",
  fontStyle: "italic",
});

export const feedMessageCardUser = style({
  marginLeft: "auto",
  background: "color-mix(in srgb, var(--accent-9) 12%, var(--gray-a2))",
  borderColor: "color-mix(in srgb, var(--accent-9) 28%, var(--gray-a4))",
});

export const feedMessageTimestamp = style({
  alignSelf: "flex-end",
  fontSize: "10px",
  color: "var(--gray-9)",
  lineHeight: 1,
  whiteSpace: "nowrap",
});

export const feedMessageMeta = style({
  fontSize: "var(--font-size-1)",
  fontWeight: "var(--font-weight-medium)",
  color: "var(--gray-10)",
  letterSpacing: "0.02em",
  textTransform: "uppercase",
});

export const startupFeedItem = style({
  display: "flex",
  alignItems: "flex-start",
  gap: "var(--space-2)",
  padding: "var(--space-3)",
  borderRadius: "var(--radius-3)",
  border: "1px solid var(--gray-a4)",
  background: "var(--gray-a2)",
  selectors: {
    '&[data-tone="error"]': {
      borderColor: "var(--red-a6)",
      background: "var(--red-a2)",
    },
  },
});

export const startupFeedBody = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-1)",
  minWidth: 0,
});

export const composerRoot = style({
  padding: "var(--space-2) var(--space-3) var(--space-3)",
  flexShrink: 0,
});

export const composerInputWrapper = style({
  position: "relative",
});

export const composerDropIndicator = style({
  position: "absolute",
  inset: 0,
  zIndex: 10,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "color-mix(in srgb, var(--accent-9) 10%, transparent)",
  border: "2px dashed var(--accent-9)",
  borderRadius: "var(--radius-3)",
  color: "var(--accent-11)",
  fontSize: "var(--font-size-2)",
  fontWeight: "var(--font-weight-medium)",
  pointerEvents: "none",
});

export const attachedImageThumbList = style({
  display: "flex",
  flexWrap: "wrap",
  gap: "var(--space-2)",
});

export const attachedImageThumbWrapper = style({
  position: "relative",
  flexShrink: 0,
});

export const attachedImageThumb = style({
  width: "56px",
  height: "56px",
  objectFit: "cover",
  borderRadius: "var(--radius-2)",
  border: "1px solid var(--gray-a5)",
  display: "block",
});

export const attachedImageRemove = style({
  position: "absolute",
  top: "-6px",
  right: "-6px",
  width: "18px",
  height: "18px",
  borderRadius: "50%",
  background: "var(--gray-12)",
  color: "var(--gray-1)",
  border: "none",
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  fontSize: "12px",
  lineHeight: 1,
  padding: 0,
});

export const composerInput = style({
  resize: "vertical",
});

export const composerActions = style({
  flexShrink: 0,
});

const composerPulse = keyframes({
  "0%, 100%": { opacity: 1 },
  "50%": { opacity: 0.3 },
});

export const composerActivityDot = style({
  width: 8,
  height: 8,
  borderRadius: "50%",
  background: "var(--accent-9)",
  animation: `${composerPulse} 1.5s ease-in-out infinite`,
});

export const fileMentionPopup = style({
  position: "absolute",
  bottom: "100%",
  left: 0,
  right: 0,
  zIndex: 50,
  marginBottom: 4,
  maxHeight: "14rem",
  overflowY: "auto",
  border: "1px solid var(--gray-a6)",
  borderRadius: "var(--radius-3)",
  background: "var(--color-panel-solid)",
  boxShadow: "var(--shadow-4)",
});

export const fileMentionItem = style({
  padding: "var(--space-2) var(--space-3)",
  cursor: "pointer",
  fontFamily: monoFontStack,
  fontSize: "var(--font-size-1)",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  selectors: {
    '&[data-selected="true"]': {
      background: "var(--accent-a3)",
    },
    "&:hover": {
      background: "var(--gray-a3)",
    },
  },
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
  overflowX: "auto",
  maxHeight: "20rem",
  overflowY: "auto",
});

export const toolCallArgumentGrid = style({
  display: "grid",
  gridTemplateColumns: "minmax(7rem, auto) minmax(0, 1fr)",
  gap: "var(--space-1) var(--space-2)",
  alignItems: "start",
});

export const toolCallLabel = style({
  color: "var(--gray-10)",
  fontFamily: monoFontStack,
});

export const toolCallValue = style({
  color: "var(--gray-12)",
  fontFamily: monoFontStack,
  whiteSpace: "pre-wrap",
  overflowWrap: "anywhere",
});

export const toolCallContentSection = style({
  color: "var(--gray-12)",
  fontSize: "var(--font-size-1)",
  lineHeight: "var(--line-height-3)",
});

globalStyle(`${toolCallContentSection} :where(p, ul, ol, blockquote)`, {
  margin: "0 0 var(--space-2)",
});

globalStyle(
  `${toolCallContentSection} :where(p:last-child, ul:last-child, ol:last-child, blockquote:last-child)`,
  {
    marginBottom: "0",
  },
);

export const terminalRoot = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
  padding: "var(--space-2)",
  borderRadius: "var(--radius-2)",
  background: "var(--gray-a2)",
  overflowX: "auto",
});

export const terminalLine = style({
  fontFamily: monoFontStack,
  fontSize: "var(--font-size-1)",
  whiteSpace: "pre-wrap",
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
  fontSize: "var(--font-size-1)",
  lineHeight: "var(--line-height-2)",
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
  fontFamily: monoFontStack,
  fontSize: "var(--font-size-1)",
  whiteSpace: "pre-wrap",
  overflowX: "auto",
});

export const steerReviewCard = style({
  margin: "var(--space-2) var(--space-4)",
  flexShrink: 0,
});

export const permissionCard = style({
  background: "var(--accent-a3)",
  border: "1px solid var(--accent-a6)",
  borderRadius: "var(--radius-3)",
  padding: "var(--space-3)",
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
});
