import { globalStyle, keyframes, style } from "@vanilla-extract/css";
import { monoFontStack } from "./global.css";

// ─── Unified feed ────────────────────────────────────────────────────────────

export const unifiedFeedRoot = style({
  display: "flex",
  flexDirection: "column",
  height: "100%",
  overflow: "hidden",
});

export const unifiedFeedScroll = style({
  flex: 1,
  overflowY: "auto",
  overscrollBehavior: "contain",
  display: "flex",
  flexDirection: "column",
  position: "relative",
});

export const scrollToBottomBtn = style({
  position: "fixed",
  bottom: "140px",
  left: "50%",
  transform: "translateX(-50%)",
  width: 36,
  height: 36,
  borderRadius: "50%",
  background: "var(--color-panel-solid)",
  border: "1px solid var(--gray-a6)",
  boxShadow: "var(--shadow-3)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "pointer",
  color: "var(--gray-11)",
  zIndex: 10,
  transition: "opacity 0.15s",
  ":hover": {
    background: "var(--gray-2)",
  },
});

export const unifiedFeedStream = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
  paddingTop: "var(--space-3)",
  paddingBottom: "var(--space-2)",
});

export const feedContentColumn = style({
  maxWidth: "640px",
  width: "100%",
  marginLeft: "auto",
  marginRight: "auto",
  paddingLeft: "var(--space-3)",
  paddingRight: "var(--space-3)",
});

export const agentAvatarWrapper = style({
  position: "relative",
  flexShrink: 0,
  alignSelf: "flex-start",
});

export const agentAvatarBadge = style({
  position: "absolute",
  bottom: -4,
  right: -4,
  width: 18,
  height: 18,
  borderRadius: "50%",
  background: "white",
  boxShadow: "0 0 0 1.5px white",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
});

export const agentAvatar = style({
  width: 40,
  height: 40,
  borderRadius: "50%",
  objectFit: "cover",
});

export const agentAvatarSpacer = style({
  width: 40,
  flexShrink: 0,
});

export const feedRowAgent = style({
  display: "flex",
  justifyContent: "flex-start",
  alignItems: "flex-start",
  gap: "var(--space-2)",
  width: "100%",
});

export const feedRowUser = style({
  display: "flex",
  justifyContent: "flex-end",
  alignItems: "flex-start",
  gap: "var(--space-2)",
});

export const userAvatar = style({
  width: 28,
  height: 28,
  borderRadius: "50%",
  flexShrink: 0,
  objectFit: "cover",
  alignSelf: "flex-start",
});

export const userAvatarSpacer = style({
  width: 28,
  flexShrink: 0,
});

export const feedBubble = style({
  padding: "var(--space-2) var(--space-3)",
  borderRadius: "20px",
  // @ts-ignore — new CSS property, not yet in types
  cornerShape: "squircle",
  border: "1px solid var(--gray-a4)",
  background: "var(--gray-a2)",
  fontSize: "var(--font-size-3)",
  lineHeight: "var(--line-height-3)",
});

export const feedBubbleMate = style({
  background: "color-mix(in srgb, var(--indigo-a3) 60%, var(--gray-a2))",
  borderColor: "var(--indigo-a4)",
});

export const feedBubbleUser = style({
  background: "color-mix(in srgb, var(--accent-9) 12%, var(--gray-a1))",
  borderColor: "var(--accent-a4)",
  borderWidth: "1px",
  borderStyle: "solid",
});

export const feedBubbleRelay = style({
  background: "color-mix(in srgb, var(--amber-a3) 50%, var(--gray-a2))",
  borderColor: "var(--amber-a4)",
  opacity: 0.85,
});

export const feedBubbleCaptain = style({
  background: "transparent",
  border: "none",
  padding: "0 !important",
});

export const feedToolGroup = style({
  maxWidth: "80%",
});

export const feedSystemMessage = style({
  display: "flex",
  justifyContent: "center",
  padding: "var(--space-1) var(--space-3)",
});

export const feedSystemMessageText = style({
  fontSize: "var(--font-size-2)",
  color: "var(--gray-9)",
});

export const taskRecapBoundary = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
  width: "auto",
  marginLeft: "calc(var(--space-3) * -1)",
  marginRight: "calc(var(--space-3) * -1)",
  padding: "var(--space-2) var(--space-3) var(--space-3)",
  borderTop: "2px solid var(--green-8)",
  borderBottom: "1px solid var(--gray-a4)",
  background: "linear-gradient(180deg, var(--green-a3) 0%, var(--gray-a2) 100%)",
});

export const taskRecapHeader = style({
  display: "flex",
  alignItems: "flex-start",
  justifyContent: "space-between",
  gap: "var(--space-3)",
  flexWrap: "wrap",
});

export const taskRecapEyebrow = style({
  display: "block",
  fontSize: "11px",
  lineHeight: 1.3,
  textTransform: "uppercase",
  letterSpacing: "0.08em",
  fontWeight: 700,
  color: "var(--green-11)",
});

export const taskRecapTitle = style({
  display: "block",
  marginTop: "2px",
  fontSize: "var(--font-size-2)",
  lineHeight: "var(--line-height-2)",
  fontWeight: 600,
  color: "var(--gray-12)",
});

export const taskRecapSummary = style({
  flexShrink: 0,
  fontFamily: monoFontStack,
  fontSize: "var(--font-size-1)",
  lineHeight: 1.5,
  color: "var(--gray-12)",
});

export const taskRecapCommitList = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-1)",
  width: "100%",
  minWidth: 0,
});

export const taskRecapCommitRow = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-1)",
  width: "100%",
  minWidth: 0,
});

export const taskRecapCommitToggle = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  width: "100%",
  minWidth: 0,
  padding: "var(--space-1) 0",
  border: "none",
  background: "transparent",
  color: "var(--gray-12)",
  textAlign: "left",
  font: "inherit",
  cursor: "pointer",
  selectors: {
    "&:hover": {
      color: "var(--gray-12)",
    },
  },
});

export const taskRecapCommitStatic = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  width: "100%",
  minWidth: 0,
  padding: "var(--space-1) 0",
});

export const taskRecapCommitHash = style({
  flexShrink: 0,
  fontFamily: monoFontStack,
  fontSize: "var(--font-size-1)",
  color: "var(--green-11)",
});

export const taskRecapCommitSubject = style({
  minWidth: 0,
  fontSize: "var(--font-size-2)",
  lineHeight: "var(--line-height-2)",
  color: "var(--gray-12)",
});

export const taskRecapCaret = style({
  flexShrink: 0,
  color: "var(--green-11)",
  transition: "transform 0.15s ease",
});

export const taskRecapDiff = style({
  width: "100%",
  overflowX: "auto",
  overflowY: "visible",
  padding: "var(--space-2)",
  borderRadius: "var(--radius-2)",
  border: "1px solid var(--gray-a4)",
  background: "var(--gray-a3)",
});

export const taskRecapDiffInner = style({
  fontFamily: monoFontStack,
  fontSize: "var(--font-size-1)",
  whiteSpace: "pre",
  textAlign: "left",
  minWidth: "max-content",
});

export const feedToolGroupHeader = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-1)",
  padding: "var(--space-1) var(--space-2)",
  borderRadius: "var(--radius-1)",
  cursor: "pointer",
  fontSize: "var(--font-size-1)",
  color: "var(--gray-10)",
  userSelect: "none",
  selectors: {
    "&:hover": {
      background: "var(--gray-a2)",
    },
  },
});

export const feedToolGroupHeaderExpanded = style({
  borderRadius: "var(--radius-2)",
  border: "1px solid var(--gray-a4)",
  background: "var(--gray-a2)",
  selectors: {
    "&:hover": {
      background: "var(--gray-a3)",
    },
  },
});

export const feedToolGroupBody = style({
  marginTop: "var(--space-1)",
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-1)",
});

const bubblePulse = keyframes({
  "0%, 100%": { opacity: 0.35 },
  "50%": { opacity: 1 },
});

const transcriptFadeIn = keyframes({
  from: { opacity: 0, transform: "translateY(6px)" },
  to: { opacity: 1, transform: "translateY(0)" },
});

export const liveBubblesRow = style({
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  paddingBottom: "var(--space-2)",
  gap: "var(--space-2)",
  flexShrink: 0,
});

export const liveBubble = style({
  display: "inline-flex",
  alignItems: "center",
  gap: 5,
  padding: "6px var(--space-3)",
  borderRadius: "var(--radius-3)",
  border: "1px solid var(--gray-a4)",
  background: "var(--gray-a2)",
});

export const thinkingBubble = style({
  display: "inline-flex",
  alignItems: "center",
  gap: 10,
  padding: "8px 0",
});

const shimmerMove = keyframes({
  from: { backgroundPosition: "200% center" },
  to: { backgroundPosition: "-200% center" },
});

export const shimmerText = style({
  background: "linear-gradient(90deg, var(--gray-10) 30%, var(--gray-1) 50%, var(--gray-10) 70%)",
  backgroundSize: "200% auto",
  WebkitBackgroundClip: "text",
  backgroundClip: "text",
  WebkitTextFillColor: "transparent",
  color: "transparent",
  animation: `${shimmerMove} 3s linear infinite`,
});

export const thinkingStopBtn = style({
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  width: 28,
  height: 28,
  borderRadius: "50%",
  background: "var(--gray-12)",
  color: "white",
  border: "none",
  cursor: "pointer",
  flexShrink: 0,
  selectors: {
    "&:hover": { background: "var(--gray-11)" },
  },
});

export const liveBubbleDot = style({
  width: 4,
  height: 4,
  borderRadius: "50%",
  background: "var(--gray-9)",
  animation: `${bubblePulse} 2s ease-in-out infinite`,
  selectors: {
    "&:nth-child(2)": { animationDelay: "0.4s" },
    "&:nth-child(3)": { animationDelay: "0.8s" },
  },
});

export const agentStateChip = style({
  display: "inline-flex",
  alignItems: "center",
  gap: "var(--space-1)",
  fontSize: "var(--font-size-1)",
  color: "var(--gray-10)",
  padding: "2px var(--space-2)",
  borderRadius: "var(--radius-2)",
  border: "1px solid var(--gray-a4)",
  background: "var(--gray-a2)",
  selectors: {
    '&[data-tone="error"]': {
      color: "var(--red-11)",
      borderColor: "var(--red-a5)",
      background: "var(--red-a2)",
    },
    '&[data-tone="warn"]': {
      color: "var(--amber-11)",
      borderColor: "var(--amber-a5)",
      background: "var(--amber-a2)",
    },
  },
});

export const sessionViewRoot = style({
  display: "flex",
  flexDirection: "column",
  height: "100%",
  overflow: "hidden",
});

export const sessionFeedColumn = style({
  flex: 1,
  minWidth: 0,
  display: "flex",
  flexDirection: "column",
  height: "100%",
  overflow: "hidden",
});

// Three-column app layout: [left: sidebar] [center: fills space] [right: agent rail]
export const appColumns = style({
  display: "grid",
  gridTemplateColumns: "260px 1fr 260px",
  maxWidth: "1400px",
  width: "100%",
  margin: "0 auto",
  flex: 1,
  minHeight: 0,
  overflow: "hidden",
  gap: "var(--space-2)",
  "@media": {
    "(max-width: 980px)": {
      gridTemplateColumns: "260px 1fr",
    },
    "(max-width: 700px)": {
      gridTemplateColumns: "1fr",
    },
  },
});

export const appColLeft = style({
  overflow: "hidden",
  "@media": {
    "(max-width: 700px)": {
      position: "absolute",
      width: 0,
      overflow: "visible",
    },
  },
});

export const appColCenter = style({
  overflow: "hidden",
  background: "var(--color-background)",
  borderLeft: "1px solid var(--gray-a5)",
  borderRight: "1px solid var(--gray-a5)",
  // boxShadow: "0 0 40px rgba(0, 0, 0, 0.08)",
});

export const appColRight = style({
  "@media": {
    "(max-width: 980px)": {
      display: "none",
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

export const agentRail = style({
  width: "100%",
  flexShrink: 0,
  overflowY: "auto",
  display: "flex",
  flexDirection: "column",
  "@media": {
    "(max-width: 768px)": {
      display: "none",
    },
  },
});

export const planPanel = style({
  padding: "var(--space-3)",
  borderTop: "1px solid var(--gray-a4)",
});

export const planStepRow = style({
  minWidth: 0,
});

export const planStepText = style({
  lineHeight: 1.4,
  minWidth: 0,
});

export const agentStatusBar = style({
  display: "none",
  "@media": {
    "(max-width: 980px)": {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-4)",
      padding: "var(--space-2) var(--space-3)",
      borderBottom: "1px solid var(--gray-a4)",
      flexShrink: 0,
    },
  },
});

export const agentStatusBarItem = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  minWidth: 0,
});

export const agentStatusBarAvatar = style({
  width: 20,
  height: 20,
  borderRadius: "50%",
  flexShrink: 0,
  objectFit: "cover",
  maskImage: "radial-gradient(circle, black 64%, transparent 64%)",
});

export const sessionTopBar = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  padding: "var(--space-2) var(--space-3)",
  borderBottom: "1px solid var(--gray-a4)",
  flexShrink: 0,
});

export const sessionTopBarLeft = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  flex: 1,
  minWidth: 0,
  overflow: "hidden",
});

export const sessionTopBarBreadcrumb = style({
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const sessionTopBarRight = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-3)",
  flexShrink: 0,
  "@media": {
    "(max-width: 699px)": { display: "none" },
    "(min-width: 980px)": { display: "none" },
  },
});

export const sessionTopBarAgentSection = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-1)",
});

export const sessionTopBarDivider = style({
  width: 1,
  height: 16,
  background: "var(--gray-a5)",
  flexShrink: 0,
});

export const sessionTopBarActions = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  flexShrink: 0,
});

// ─── SessionHeader ────────────────────────────────────────────────────────────

export const sessionHeaderRoot = style({
  flexShrink: 0,
  borderBottom: "1px solid var(--gray-a4)",
  position: "relative",
});

export const sessionHeaderRow1 = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-1)",
  padding: "var(--space-2) var(--space-2) var(--space-1) var(--space-3)",
});

export const sessionHeaderTitle = style({
  flex: 1,
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const sessionHeaderRow2 = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  padding: "var(--space-1) var(--space-3) var(--space-2) var(--space-3)",
  width: "100%",
  border: 0,
  borderTop: "1px solid var(--gray-a3)",
  background: "transparent",
  color: "inherit",
  textAlign: "left",
  cursor: "pointer",
  selectors: {
    "&:hover": {
      background: "var(--gray-a2)",
    },
  },
});

export const sessionHeaderRow2Title = style({
  flex: 1,
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const sessionHeaderExpanded = style({
  borderTop: "1px solid var(--gray-a4)",
  background: "var(--gray-a1)",
  maxHeight: 0,
  overflow: "hidden",
  transition: "max-height 0.22s ease",
  selectors: {
    '&[data-open="true"]': {
      maxHeight: "28rem",
      overflowY: "auto",
    },
  },
});

export const sessionHeaderPanelInner = style({
  padding: "var(--space-3)",
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-4)",
});

export const sessionHeaderSectionLabel = style({
  fontSize: "var(--font-size-1)",
  letterSpacing: "0.07em",
  textTransform: "uppercase",
  color: "var(--gray-10)",
  marginBottom: "var(--space-2)",
});

export const sessionHeaderAgentRow = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  flexWrap: "wrap",
});

export const sessionHeaderAgentLabel = style({
  fontSize: "var(--font-size-1)",
  color: "var(--gray-10)",
  width: 46,
  flexShrink: 0,
});

export const sessionHeaderBranchMeta = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  flexWrap: "wrap",
});

export const sessionHeaderCaret = style({
  color: "var(--gray-10)",
  flexShrink: 0,
});

export const sessionHeaderProgressFlex = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-1)",
  flexShrink: 0,
});

export const sessionHeaderDot = style({
  width: 7,
  height: 7,
  borderRadius: "999px",
  background: "var(--gray-6)",
  flexShrink: 0,
  selectors: {
    '&[data-complete="true"]': {
      background: "var(--accent-9)",
    },
  },
});

export const sessionHeaderDiffFlex = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-1)",
  flexShrink: 0,
});

export const sessionHeaderDiffAdd = style({
  color: "var(--green-10)",
  fontFamily: "var(--code-font-family)",
});

export const sessionHeaderDiffRemove = style({
  color: "var(--red-10)",
  fontFamily: "var(--code-font-family)",
});

export const sessionHeaderStepIconWrap = style({
  paddingTop: 2,
  display: "flex",
  flexShrink: 0,
});

export const sessionHeaderStepText = style({
  color: "var(--gray-12)",
  selectors: {
    '&[data-status="Completed"]': {
      color: "var(--gray-9)",
      textDecoration: "line-through",
    },
    '&[data-status="Failed"]': {
      color: "var(--red-11)",
    },
  },
});

export const sessionHeaderHistoryItem = style({
  borderTop: "1px solid var(--gray-a4)",
  minWidth: 0,
  display: "flex",
  flexDirection: "column",
});

export const sessionHeaderHistoryBtn = style({
  display: "flex",
  alignItems: "flex-start",
  gap: "var(--space-2)",
  width: "100%",
  padding: "var(--space-2) 0",
  border: 0,
  background: "transparent",
  color: "inherit",
  textAlign: "left",
  cursor: "pointer",
});

export const sessionHeaderHistoryTitleRow = style({
  flex: 1,
  minWidth: 0,
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  flexWrap: "wrap",
});

export const sessionHeaderHistoryTitle = style({
  flex: 1,
  minWidth: 0,
  lineHeight: 1.35,
});

export const sessionHeaderHistoryBody = style({
  paddingLeft: "calc(11px + var(--space-2))",
  paddingBottom: "var(--space-2)",
});

export const sessionHeaderHistoryCaret = style({
  color: "var(--gray-10)",
  flexShrink: 0,
  marginTop: 3,
});

export const agentHeader = style({
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
  padding: "var(--space-3) var(--space-3) var(--space-2)",
  borderBottom: "1px solid var(--gray-a4)",
  flexShrink: 0,
});

export const agentHeaderMain = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  minWidth: 0,
});

export const agentHeaderAvatar = style({
  width: 32,
  height: 32,
  borderRadius: "50%",
  flexShrink: 0,
  objectFit: "cover",
  maskImage: "radial-gradient(circle, black 64%, transparent 64%)",
});

export const agentHeaderAvatarFallback = style({
  width: 32,
  height: 32,
  borderRadius: "50%",
  flexShrink: 0,
  display: "grid",
  placeItems: "center",
  background: "var(--gray-a3)",
});

export const agentHeaderBody = style({
  display: "flex",
  flexDirection: "column",
  gap: "1px",
  flex: 1,
  minWidth: 0,
});

export const agentHeaderSummaryRow = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  width: "100%",
  minWidth: 0,
  overflow: "hidden",
});

export const agentHeaderRole = style({
  flex: "1 1 auto",
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const agentHeaderControlRow = style({
  display: "flex",
  alignItems: "center",
  gap: "4px",
  width: "100%",
  minWidth: 0,
  overflow: "hidden",
  whiteSpace: "nowrap",
  flexWrap: "nowrap",
  selectors: {
    [`${sessionHeaderAgentRow} &`]: {
      width: "auto",
    },
  },
});

export const agentHeaderPickerTrigger = style({
  display: "block",
  minWidth: 0,
  maxWidth: "100%",
  padding: 0,
  border: "none",
  background: "transparent",
  color: "inherit",
  font: "inherit",
  textAlign: "left",
  cursor: "pointer",
});

export const agentHeaderPickerText = style({
  display: "block",
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  cursor: "pointer",
  textDecoration: "underline dotted",
  textUnderlineOffset: "2px",
});

export const agentHeaderPickerTextGrow = style({
  flex: "1 1 auto",
  minWidth: 0,
});

export const agentHeaderPickerStatic = style({
  display: "block",
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
});

export const agentHeaderSlash = style({
  flexShrink: 0,
});

export const agentHeaderContext = style({
  width: 18,
  height: 18,
  flexShrink: 0,
  display: "grid",
  placeItems: "center",
  marginLeft: "auto",
  color: "var(--accent-10)",
  selectors: {
    '&[data-tone="low"]': {
      color: "var(--red-10)",
    },
  },
});

export const agentHeaderContextSvg = style({
  width: "100%",
  height: "100%",
  transform: "rotate(-90deg)",
  overflow: "visible",
});

export const agentHeaderContextTrack = style({
  fill: "none",
  stroke: "var(--gray-a4)",
  strokeWidth: 2.5,
});

export const agentHeaderContextArc = style({
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2.5,
  strokeLinecap: "round",
  transition: "stroke-dashoffset 160ms ease",
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

export const feedTimestamp = style({
  fontSize: "var(--font-size-1)",
  color: "var(--gray-9)",
});

export const feedBubbleCol = style({
  display: "flex",
  flexDirection: "column",
  width: "100%",
});

export const feedBubbleColUser = style({
  maxWidth: "85%",
});

export const feedMessageTimestamp = style({
  alignSelf: "flex-end",
  fontSize: "10px",
  color: "var(--gray-9)",
  lineHeight: 1,
  whiteSpace: "nowrap",
  float: "right",
  marginTop: "6px",
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
  padding: "var(--space-2)",
  paddingBottom: "max(var(--space-3), env(safe-area-inset-bottom, 0px))",
  flexShrink: 0,
});

export const composerInputWrapper = style({
  position: "relative",
  flex: 1,
  minWidth: 0,
});

export const transcriptPreview = style({
  maxHeight: "4.5em",
  overflowY: "auto",
  fontSize: "var(--font-size-2)",
  lineHeight: "var(--line-height-2)",
  color: "var(--gray-11)",
  padding: "4px 0",
  textAlign: "center",
  maxWidth: "65%",
  margin: "0 auto",
  maskImage: "linear-gradient(to bottom, transparent 0%, black 30%, black 100%)",
  WebkitMaskImage: "linear-gradient(to bottom, transparent 0%, black 30%, black 100%)",
  animation: `${transcriptFadeIn} 0.25s ease-out`,
  scrollbarWidth: "none",
  selectors: {
    "&::-webkit-scrollbar": {
      display: "none",
    },
  },
});

export const composerOverlay = style({
  position: "absolute",
  top: 0,
  bottom: 0,
  left: "50%",
  transform: "translateX(-50%)",
  width: "65%",
  zIndex: 1,
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  padding: "4px 12px",
  pointerEvents: "none",
  maskImage: "linear-gradient(to bottom, transparent 0%, black 30%, black 100%)",
  WebkitMaskImage: "linear-gradient(to bottom, transparent 0%, black 30%, black 100%)",
  "@media": {
    "(max-width: 700px)": {
      left: 50,
      right: 90,
      width: "auto",
      transform: "none",
    },
  },
});

export const composerInlineBtn = style({
  position: "absolute",
  top: "50%",
  transform: "translateY(-50%)",
  zIndex: 2,
  width: 32,
  height: 32,
  borderRadius: "50%",
  border: "none",
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: 0,
  transition: "background 0.1s, opacity 0.1s",
  background: "transparent",
  color: "var(--gray-11)",
  ":hover": {
    background: "var(--gray-a3)",
  },
  selectors: {
    "&:disabled": {
      opacity: 0.3,
      cursor: "default",
    },
    '&[data-variant="solid"]': {
      background: "var(--accent-9)",
      color: "var(--accent-contrast)",
    },
    '&[data-variant="solid"]:hover': {
      opacity: 0.85,
    },
    '&[data-pos="left"]': {
      left: 6,
    },
    '&[data-pos="right"]': {
      right: 6,
    },
    '&[data-pos="right-2"]': {
      right: 44,
    },
  },
  "@media": {
    "(max-width: 700px)": {
      width: 40,
      height: 40,
    },
  },
});

export const composerEscHint = style({
  display: "inline-flex",
  "@media": {
    "(max-width: 700px)": {
      display: "none",
    },
  },
});

export const pageDropOverlay = style({
  position: "fixed",
  inset: 0,
  zIndex: 1000,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "color-mix(in srgb, var(--accent-9) 10%, transparent)",
  border: "3px dashed var(--accent-9)",
  borderRadius: "var(--radius-4)",
  color: "var(--accent-11)",
  fontSize: "var(--font-size-4)",
  fontWeight: "var(--font-weight-medium)",
  pointerEvents: "none",
  margin: "var(--space-3)",
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

export const feedImageThumb = style({
  maxWidth: "160px",
  maxHeight: "120px",
  objectFit: "cover",
  borderRadius: "var(--radius-2)",
  cursor: "pointer",
  display: "block",
});

export const feedImageLightboxOverlay = style({
  position: "fixed",
  inset: 0,
  background: "rgba(0,0,0,0.85)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  zIndex: 1000,
  cursor: "zoom-out",
});

export const feedImageLightboxImg = style({
  maxWidth: "90vw",
  maxHeight: "90vh",
  objectFit: "contain",
  borderRadius: "var(--radius-2)",
});

export const composerInput = style({
  resize: "none",
  borderRadius: "12px",
  paddingLeft: 42,
  paddingRight: 42,
  background: "transparent",
  "@media": {
    "(max-width: 700px)": {
      paddingLeft: 48,
      paddingRight: 48,
    },
  },
});

export const composerInputWideRight = style({
  paddingRight: 80,
  "@media": {
    "(max-width: 700px)": {
      paddingRight: 88,
    },
  },
});

export const composerStatusRow = style({
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  justifyContent: "flex-start",
  width: "100%",
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
    '&[data-special="mate"]': {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-2)",
      color: "var(--accent-11)",
      fontFamily: "inherit",
    },
  },
});

globalStyle(`${composerInput}, ${composerInput} textarea`, {
  background: "transparent",
  minHeight: "unset",
});

globalStyle(`${composerInputWrapper}[data-target="mate"] textarea`, {
  outline: "2px solid var(--accent-9)",
  outlineOffset: "-1px",
});

export const toolCallBlock = style({
  fontSize: "var(--font-size-1)",
});

export const toolCallBlockExpanded = style({
  borderRadius: "var(--radius-2)",
  border: "1px solid var(--gray-a4)",
  overflow: "hidden",
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

export const toolCallHeaderCollapsed = style({
  borderRadius: "var(--radius-1)",
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
  fontSize: "var(--font-size-3)",
  lineHeight: "var(--line-height-3)",
});

export const bubbleContent = style({});

export const bubbleContentCollapsed = style({
  maxHeight: 400,
  overflow: "hidden",
  maskImage: "linear-gradient(to bottom, black 300px, transparent 400px)",
  WebkitMaskImage: "linear-gradient(to bottom, black 300px, transparent 400px)",
});

export const bubbleCopyBtn = style({
  position: "absolute",
  top: 0,
  right: 0,
  opacity: 0,
  transition: "opacity 0.15s",
});

export const bubbleActions = style({
  display: "flex",
  flexDirection: "row",
  gap: "var(--space-2)",
  alignSelf: "flex-start",
  flexShrink: 0,
});

export const feedBubbleWithActions = style({
  display: "flex",
  flexDirection: "column",
  alignItems: "flex-start",
  gap: "var(--space-2)",
  flex: 1,
  minWidth: 0,
  marginBottom: "var(--space-4)",
});

export const feedBubbleWithActionsUser = style({
  justifyContent: "flex-end",
});

globalStyle(`${feedBubbleWithActions}:hover ${bubbleActions}`, {
  opacity: 1,
});

globalStyle(`${textBlockRoot} :where(p, ul, ol, blockquote)`, {
  margin: "0 0 var(--space-3)",
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

globalStyle(`${textBlockRoot} a`, {
  color: "var(--gray-12)",
  textDecoration: "underline",
  textDecorationColor: "var(--gray-a7)",
  textUnderlineOffset: "2px",
});

globalStyle(`${textBlockRoot} blockquote`, {
  borderLeft: "3px solid var(--gray-a6)",
  paddingLeft: "var(--space-3)",
  color: "var(--gray-11)",
});

globalStyle(`${textBlockRoot} table`, {
  borderCollapse: "collapse",
  width: "100%",
  margin: "var(--space-2) 0",
  fontSize: "var(--font-size-2)",
});

globalStyle(`${textBlockRoot} :where(th, td)`, {
  border: "1px solid var(--gray-a5)",
  padding: "var(--space-1) var(--space-2)",
  textAlign: "left",
});

globalStyle(`${textBlockRoot} th`, {
  background: "var(--gray-a3)",
  fontWeight: "var(--font-weight-medium)",
});

export const textBlockCodeBlock = style({
  overflow: "hidden",
  borderRadius: "var(--radius-2)",
  fontSize: "var(--font-size-1)",
  border: "1px solid var(--gray-a3)",
  padding: "var(--space-1) var(--space-2)",
  margin: "var(--space-2) 0",
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

// ─── Task description prose ───────────────────────────────────────────────────
// Mirrors textBlockRoot's conventions at font-size-2 / line-height-2

export const taskDescriptionRoot = style({
  fontSize: "var(--font-size-2)",
  lineHeight: "var(--line-height-2)",
  color: "var(--gray-12)",
});

globalStyle(`${taskDescriptionRoot} :where(p, ul, ol, blockquote)`, {
  margin: "0 0 var(--space-2)",
});

globalStyle(
  `${taskDescriptionRoot} :where(p:last-child, ul:last-child, ol:last-child, blockquote:last-child)`,
  {
    marginBottom: "0",
  },
);

globalStyle(`${taskDescriptionRoot} :where(ul, ol)`, {
  paddingLeft: "var(--space-4)",
});

globalStyle(`${taskDescriptionRoot} :where(pre)`, {
  margin: 0,
});

globalStyle(`${taskDescriptionRoot} a`, {
  color: "var(--gray-12)",
  textDecoration: "underline",
  textDecorationColor: "var(--gray-a7)",
  textUnderlineOffset: "2px",
});

globalStyle(`${taskDescriptionRoot} blockquote`, {
  borderLeft: "3px solid var(--gray-a6)",
  paddingLeft: "var(--space-3)",
  color: "var(--gray-11)",
});

globalStyle(`${taskDescriptionRoot} table`, {
  borderCollapse: "collapse",
  width: "100%",
  margin: "var(--space-2) 0",
  fontSize: "var(--font-size-1)",
});

globalStyle(`${taskDescriptionRoot} :where(th, td)`, {
  border: "1px solid var(--gray-a5)",
  padding: "var(--space-1) var(--space-2)",
  textAlign: "left",
});

globalStyle(`${taskDescriptionRoot} th`, {
  background: "var(--gray-a3)",
  fontWeight: "var(--font-weight-medium)",
});

globalStyle(`${taskDescriptionRoot} code:not(pre > code)`, {
  fontFamily: monoFontStack,
  fontSize: "0.875em",
  background: "var(--gray-a3)",
  borderRadius: "var(--radius-1)",
  padding: "0.1em 0.3em",
});

export const steerReviewCard = style({
  margin: "var(--space-2) var(--space-4)",
  flexShrink: 0,
});

export const humanReviewCard = style({
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

export const hamburgerBtn = style({
  // !important needed to override Radix IconButton's display: inline-flex
  display: "none !important" as "none",
  "@media": {
    "(max-width: 700px)": {
      display: "flex !important" as "flex",
    },
  },
});

export const floatingHamburger = style({
  display: "none",
  "@media": {
    "(max-width: 700px)": {
      display: "flex",
      position: "fixed",
      top: "var(--space-2)",
      left: "var(--space-2)",
      zIndex: 100,
    },
  },
});

export const floatingTaskBtn = style({
  display: "none",
  "@media": {
    "(max-width: 700px)": {
      display: "flex",
      position: "fixed",
      top: "var(--space-2)",
      right: "var(--space-2)",
      zIndex: 100,
    },
  },
});

export const taskPanelBackdrop = style({
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

export const taskPanelRoot = style({
  display: "none",
  "@media": {
    "(max-width: 700px)": {
      display: "flex",
      flexDirection: "column",
      position: "fixed",
      right: 0,
      top: 0,
      bottom: 0,
      width: 300,
      zIndex: 200,
      background: "var(--color-background)",
      transform: "translateX(100%)",
      transition: "transform 0.2s ease",
      borderLeft: "1px solid var(--gray-a6)",
      boxShadow: "-4px 0 16px rgba(0,0,0,0.2)",
      overflowY: "auto",
    },
  },
  selectors: {
    '&[data-open="true"]': {
      transform: "translateX(0)",
    },
  },
});

// Override agentRail's display:none inside the task panel overlay
globalStyle(`${taskPanelRoot} .${agentRail}`, {
  display: "flex",
});
