import { Fragment, memo, useEffect, useMemo, useRef, useState } from "react";
import { Box, Flex, Spinner, Text } from "@radix-ui/themes";
import { ArrowDown, CaretRight, Stop } from "@phosphor-icons/react";
import { encode } from "gpt-tokenizer";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import { getShipClient } from "../api/client";
import type { AgentSnapshot, ContentBlock, Role, SessionStartupState } from "../generated/ship";
import { useDocumentDrop } from "../hooks/useDocumentDrop";
import type { BlockEntry } from "../state/blockStore";
import {
  diffAdd,
  diffContext,
  diffRemove,
  feedBubble,
  feedBubbleActivitySummary,
  feedBubbleCaptain,
  feedBubbleCol,
  feedBubbleColUser,
  feedBubbleMate,
  feedBubbleRelay,
  feedBubbleSelected,
  feedBubbleSteer,
  feedBubbleUser,
  feedContentColumn,
  feedImageUser,
  feedMessageMeta,
  feedRowAgent,
  feedRowAnimate,
  feedRowUser,
  feedTimeGap,
  feedSystemMessage,
  feedSystemMessageText,
  liveBubbleDot,
  liveBubbleSlot,
  liveBubblesRow,
  scrollToBottomBtn,
  shimmerText,
  startupFeedBody,
  startupFeedItem,
  taskRecapBoundary,
  taskRecapBoundaryAccepted,
  taskRecapBoundaryError,
  taskRecapBoundaryNeutral,
  taskRecapCaret,
  taskRecapCommitHash,
  taskRecapCommitList,
  taskRecapCommitRow,
  taskRecapCommitStatic,
  taskRecapCommitSubject,
  taskRecapCommitToggle,
  taskRecapContent,
  taskRecapDiff,
  taskRecapDiffInner,
  taskRecapEyebrow,
  taskRecapHeader,
  taskRecapSummary,
  taskRecapTitle,
  thinkingAvatarBtn,
  thinkingAvatarImg,
  thinkingAvatarStop,
  thinkingBubble,
  unifiedFeedRoot,
  unifiedFeedScroll,
  unifiedFeedStream,
} from "../styles/session-view.css";
import { AgentKindIcon } from "./AgentKindIcon";
import { BubbleActionBar } from "./blocks/BubbleActionBar";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { ImageBlock } from "./blocks/ImageBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import { TextBlock } from "./blocks/TextBlock";

const GAP_MS = 2 * 60 * 1000;
const MAX_RENDERED_BLOCKS = 80;
const TOKEN_CACHE_LIMIT = 500;
const NOOP_IMAGE_DROP = () => undefined;

const tokenCache = new Map<string, number>();
const structuredTokenCache = new WeakMap<object, number>();

function rememberTokenCount(key: string, count: number): number {
  tokenCache.set(key, count);
  if (tokenCache.size > TOKEN_CACHE_LIMIT) {
    const oldestKey = tokenCache.keys().next().value;
    if (oldestKey !== undefined) {
      tokenCache.delete(oldestKey);
    }
  }
  return count;
}

function countTokens(text: string): number {
  const cached = tokenCache.get(text);
  if (cached !== undefined) {
    tokenCache.delete(text);
    tokenCache.set(text, cached);
    return cached;
  }
  return rememberTokenCount(text, encode(text).length);
}

function countStructuredTokens(value: unknown): number {
  if (value == null) return 0;
  if (typeof value === "string") return countTokens(value);
  if (typeof value === "object") {
    const cached = structuredTokenCache.get(value);
    if (cached !== undefined) return cached;
    const count = countTokens(JSON.stringify(value));
    structuredTokenCache.set(value, count);
    return count;
  }
  return countTokens(String(value));
}

type TextBlockType = Extract<ContentBlock, { tag: "Text" }>;
type ToolCallBlockType = Extract<ContentBlock, { tag: "ToolCall" }>;
type TaskRecapBlockType = Extract<ContentBlock, { tag: "TaskRecap" }>;
type WorkflowMilestoneBlockType = Extract<ContentBlock, { tag: "WorkflowMilestone" }>;
type PhaseBreakTone = "accepted" | "error" | "neutral";

type SingleSegment = { kind: "single"; entry: BlockEntry };
type FeedSegment = SingleSegment;

type TurnStats = {
  tokens: number;
  ok: number;
  failed: number;
  lastUtterance: string;
};

type RenderedSegment = {
  seg: FeedSegment;
  agentForBlock: AgentSnapshot | null;
  isTaskRecap: boolean;
  gapLabel: string | null;
  timestampMs: number | null;
};

function parseTimestampMs(iso: string | null | undefined): number | null {
  if (!iso) return null;
  const timestampMs = Date.parse(iso);
  return Number.isNaN(timestampMs) ? null : timestampMs;
}

function formatRelativeTime(timestampMs: number, now: number): string {
  const diffMs = now - timestampMs;
  const seconds = Math.floor(diffMs / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes === 1) return "1 minute ago";
  if (minutes < 60) return `${minutes} minutes ago`;
  const hours = Math.floor(minutes / 60);
  if (hours === 1) return "1 hour ago";
  if (hours < 24) return `${hours} hours ago`;
  if (hours < 48) return "yesterday";
  const days = Math.floor(hours / 24);
  return `${days} days ago`;
}

// ─── Synthetic human-text detection ───────────────────────────────────────────
// Human blocks injected by the server keep the text wire format, but some are
// tagged synthetic entries that should render differently from real user text.

type SyntheticHumanText =
  | { kind: "mateActivitySummary"; body: string }
  | { kind: "mateUpdate"; body: string }
  | { kind: "systemNotification"; body: string };

const syntheticHumanPatterns: Array<{
  kind: SyntheticHumanText["kind"];
  regex: RegExp;
}> = [
  {
    kind: "mateActivitySummary",
    regex: /<mate-activity-summary>\n?([\s\S]*?)\n?<\/mate-activity-summary>/,
  },
  {
    kind: "mateUpdate",
    regex: /<mate-update>\n?([\s\S]*?)\n?<\/mate-update>/,
  },
  {
    kind: "systemNotification",
    regex: /<system-notification>\n?([\s\S]*?)\n?<\/system-notification>/,
  },
];

function parseSyntheticHumanText(block: TextBlockType): SyntheticHumanText | null {
  if (block.source.tag !== "Human") return null;
  for (const pattern of syntheticHumanPatterns) {
    const match = block.text.match(pattern.regex);
    if (match) {
      return { kind: pattern.kind, body: match[1].trim() };
    }
  }
  return null;
}


function formatDuration(totalSeconds: number): string {
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const mins = Math.floor(totalSeconds / 60);
  const secs = totalSeconds % 60;
  if (secs === 0) return `${mins}m`;
  return `${mins}m ${secs}s`;
}

function phaseBreakToneClassName(tone: PhaseBreakTone): string {
  switch (tone) {
    case "accepted":
      return taskRecapBoundaryAccepted;
    case "error":
      return taskRecapBoundaryError;
    case "neutral":
      return taskRecapBoundaryNeutral;
  }
}

function workflowMilestoneTone(kind: WorkflowMilestoneBlockType["kind"]): PhaseBreakTone {
  switch (kind.tag) {
    case "RebaseConflict":
      return "error";
    case "PlanSet":
    case "StepCommitted":
    case "ReviewSubmitted":
      return "neutral";
  }
}

// ─── Feed segmentation ────────────────────────────────────────────────────────

function buildSegments(blocks: BlockEntry[], debugMode: boolean): FeedSegment[] {
  const visible = blocks.filter(
    (b) =>
      b.block.tag !== "PlanUpdate" &&
      (debugMode || b.block.tag !== "ToolCall") &&
      !(b.block.tag === "Text" && b.block.source.tag === "AgentThought") &&
      !(b.block.tag === "Text" && b.block.text.trim() === "") &&
      !(b.block.tag === "Permission" && b.block.resolution?.tag === "Approved") &&
      (b.role.tag !== "Mate" ||
        (b.block.tag === "Text" && (b.block.source.tag === "Human" || b.block.source.tag === "Steer"))),
  );
  return visible.map((entry) => ({ kind: "single", entry }));
}

function segmentAgentRole(seg: FeedSegment): Role | null {
  const { block, role } = seg.entry;
  if (block.tag === "Text" && block.source.tag === "Steer") {
    return { tag: "Captain" };
  }
  if (block.tag === "Text") {
    const syntheticHuman = parseSyntheticHumanText(block);
    if (syntheticHuman) return null;
    if (block.source.tag === "Human") {
      if (role.tag === "Captain") return null;
      return { tag: "Captain" };
    }
  }
  if (block.tag === "Image" && role.tag === "Captain") {
    return null;
  }
  if (block.tag === "TaskRecap") return null;
  return role;
}

function computeToolCallTokenCount(block: ToolCallBlockType): number {
  let tokens = countTokens(block.arguments) + countStructuredTokens(block.raw_output);

  for (const content of block.content) {
    switch (content.tag) {
      case "Text":
        tokens += countTokens(content.text);
        break;
      case "Diff":
        tokens += countTokens(content.unified_diff);
        break;
      case "Terminal":
        if (content.snapshot?.output) {
          tokens += countTokens(content.snapshot.output);
        }
        break;
      case "Raw":
        tokens += countStructuredTokens(content.data);
        break;
    }
  }

  if (block.error) {
    tokens += countTokens(block.error.message);
    tokens += countStructuredTokens(block.error.details);
  }

  return tokens;
}

function computeTurnStats(
  blocks: BlockEntry[],
  roleTag: "Captain" | "Mate",
  turnStartedAt?: string | null,
): TurnStats {
  let tokens = 0;
  let ok = 0;
  let failed = 0;
  let lastUtterance = "";
  const turnStartMs = parseTimestampMs(turnStartedAt);
  let turnStarted = turnStartMs == null;
  let lastMsgIdx = -1;

  for (let i = blocks.length - 1; i >= 0; i--) {
    const blockEntry = blocks[i];
    if (
      blockEntry.role.tag === roleTag &&
      blockEntry.block.tag === "Text" &&
      blockEntry.block.source.tag === "AgentMessage"
    ) {
      lastMsgIdx = i;
      break;
    }
  }

  const turnStartIndex = lastMsgIdx >= 0 ? lastMsgIdx : 0;
  for (const blockEntry of blocks.slice(turnStartIndex)) {
    if (!turnStarted) {
      const blockMs = parseTimestampMs(blockEntry.timestamp);
      if (blockMs != null && blockMs >= turnStartMs) {
        turnStarted = true;
      } else {
        continue;
      }
    }

    if (blockEntry.role.tag !== roleTag) continue;
    if (blockEntry.block.tag === "Text") {
      if (blockEntry.block.source.tag === "AgentThought") {
        tokens += countTokens(blockEntry.block.text);
        lastUtterance = blockEntry.block.text;
      } else if (blockEntry.block.source.tag === "AgentMessage") {
        tokens += countTokens(blockEntry.block.text);
      }
    } else if (blockEntry.block.tag === "ToolCall") {
      if (blockEntry.block.status.tag === "Success") ok++;
      else if (blockEntry.block.status.tag === "Failure") failed++;
      tokens += computeToolCallTokenCount(blockEntry.block);
    }
  }

  if (!lastUtterance && lastMsgIdx >= 0) {
    const lastMessage = blocks[lastMsgIdx];
    if (lastMessage.block.tag === "Text") {
      lastUtterance = lastMessage.block.text;
    }
  }

  return { tokens, ok, failed, lastUtterance };
}

// ─── TaskRecap components ──────────────────────────────────────────────────────

function CommitDiffView({ diff }: { diff: string }) {
  return (
    <Box
      className={taskRecapDiff}
      data-testid="task-recap-diff"
      data-diff-flow="inline"
      style={{ marginTop: "var(--space-1)" }}
    >
      <Box className={taskRecapDiffInner}>
        {diff.split("\n").map((line, index) => {
          if (line.startsWith("+") && !line.startsWith("+++")) {
            return (
              <span key={index} className={diffAdd}>
                {line}
              </span>
            );
          }
          if (line.startsWith("-") && !line.startsWith("---")) {
            return (
              <span key={index} className={diffRemove}>
                {line}
              </span>
            );
          }
          return (
            <span key={index} className={diffContext}>
              {line}
            </span>
          );
        })}
      </Box>
    </Box>
  );
}

function TaskRecapBlock({
  block,
  duration,
}: {
  block: TaskRecapBlockType;
  duration: number | null;
}) {
  const [expandedHashes, setExpandedHashes] = useState<Set<string>>(() => new Set());
  const { commits, stats } = block;

  function toggleExpanded(hash: string) {
    setExpandedHashes((prev) => {
      const next = new Set(prev);
      if (next.has(hash)) next.delete(hash);
      else next.add(hash);
      return next;
    });
  }

  return (
    <Box
      className={`${taskRecapBoundary} ${phaseBreakToneClassName("accepted")}`}
      data-testid="task-recap-boundary"
      data-feed-boundary="phase-break"
      data-phase-break-tone="accepted"
    >
      <Box className={taskRecapContent}>
        <Flex className={taskRecapHeader} align="start" justify="between" gap="3">
          <Box style={{ minWidth: 0 }}>
            <Text className={taskRecapEyebrow}>Phase break</Text>
            <Text className={taskRecapTitle}>Previous task accepted</Text>
            {duration != null && (
              <Text style={{ fontSize: "var(--font-size-1)", color: "var(--gray-9)" }}>
                Completed in {formatDuration(duration)}
              </Text>
            )}
          </Box>
          {stats && (
            <Text className={taskRecapSummary}>
              <span style={{ color: "var(--green-11)" }}>+{stats.insertions}</span>{" "}
              <span style={{ color: "var(--red-11)" }}>−{stats.deletions}</span> across{" "}
              {stats.files_changed} file{stats.files_changed !== 1 ? "s" : ""}
            </Text>
          )}
        </Flex>
        {commits.length > 0 && (
          <Box className={taskRecapCommitList}>
            {commits.map((commit) => {
              const expanded = expandedHashes.has(commit.hash);
              const commitLine = (
                <>
                  {commit.diff && (
                    <CaretRight
                      size={12}
                      className={taskRecapCaret}
                      style={{ transform: expanded ? "rotate(90deg)" : "rotate(0deg)" }}
                    />
                  )}
                  <Text className={taskRecapCommitHash}>{commit.hash}</Text>
                  <Text className={taskRecapCommitSubject}>{commit.subject}</Text>
                </>
              );

              return (
                <Box key={commit.hash} className={taskRecapCommitRow}>
                  {commit.diff ? (
                    <button
                      type="button"
                      className={taskRecapCommitToggle}
                      onClick={() => toggleExpanded(commit.hash)}
                    >
                      {commitLine}
                    </button>
                  ) : (
                    <Flex className={taskRecapCommitStatic} align="center" gap="2">
                      {commitLine}
                    </Flex>
                  )}
                  {expanded && commit.diff && <CommitDiffView diff={commit.diff} />}
                </Box>
              );
            })}
          </Box>
        )}
      </Box>
    </Box>
  );
}

function WorkflowMilestoneBlock({ block }: { block: WorkflowMilestoneBlockType }) {
  const tone = workflowMilestoneTone(block.kind);

  return (
    <Box
      className={`${taskRecapBoundary} ${phaseBreakToneClassName(tone)}`}
      data-testid="workflow-milestone-boundary"
      data-feed-boundary="phase-break"
      data-phase-break-kind={block.kind.tag}
      data-phase-break-tone={tone}
    >
      <Box className={taskRecapContent}>
        <Flex className={taskRecapHeader} align="start" justify="between" gap="3">
          <Box style={{ minWidth: 0 }}>
            <Text className={taskRecapEyebrow}>Phase break</Text>
            <Text className={taskRecapTitle}>{block.title}</Text>
            {block.summary && (
              <Text style={{ fontSize: "var(--font-size-1)", color: "var(--gray-9)" }}>
                {block.summary}
              </Text>
            )}
          </Box>
        </Flex>
        {block.items.length > 0 && (
          <Box className={taskRecapCommitList}>
            {block.items.map((item, index) => (
              <Box key={`${index}-${item}`} className={taskRecapCommitRow}>
                <Text className={taskRecapCommitSubject}>{item}</Text>
              </Box>
            ))}
          </Box>
        )}
      </Box>
    </Box>
  );
}

// ─── Raw block debug ──────────────────────────────────────────────────────────

function RawBlockDebug({ entry }: { entry: BlockEntry }) {
  const [expanded, setExpanded] = useState(false);
  const json = useMemo(
    () => JSON.stringify({ blockId: entry.blockId, role: entry.role, block: entry.block }, null, 2),
    [entry.blockId, entry.role, entry.block],
  );

  return (
    <Box
      px="2"
      pt="1"
      pb="2"
      style={{ border: "1px dashed var(--gray-a5)", borderRadius: "var(--radius-2)" }}
    >
      <Text size="1" color="gray">
        raw block
      </Text>
      <Box
        style={{
          fontFamily: "monospace",
          fontSize: "var(--font-size-1)",
          whiteSpace: "pre-wrap",
          marginTop: "var(--space-1)",
          background: "var(--gray-a2)",
          borderRadius: "var(--radius-1)",
          padding: "var(--space-1)",
          maxHeight: expanded ? undefined : "8rem",
          overflow: expanded ? "visible" : "hidden",
        }}
      >
        {json}
      </Box>
      <button
        type="button"
        onClick={() => setExpanded((value) => !value)}
        style={{
          all: "unset",
          cursor: "pointer",
          fontSize: "var(--font-size-1)",
          color: "var(--gray-10)",
          marginTop: "var(--space-1)",
          display: "block",
        }}
      >
        {expanded ? "▲ collapse" : "▼ expand"}
      </button>
    </Box>
  );
}

// ─── Single block ─────────────────────────────────────────────────────────────

const ToolCallDebugBlock = memo(function ToolCallDebugBlock({
  block,
  role,
}: {
  block: ToolCallBlockType;
  role: Role;
}) {
  const [expanded, setExpanded] = useState(false);
  const statusColor =
    block.status.tag === "Success"
      ? "var(--green-9)"
      : block.status.tag === "Failure"
        ? "var(--red-9)"
        : "var(--gray-9)";
  const prettyArgs = useMemo(() => {
    try {
      return JSON.stringify(JSON.parse(block.arguments), null, 2);
    } catch {
      return block.arguments;
    }
  }, [block.arguments]);
  const hasArgs = block.arguments !== "" && block.arguments !== "{}";

  return (
    <Box
      px="2"
      py="1"
      style={{
        borderLeft: `2px solid ${statusColor}`,
        background: "var(--gray-a2)",
        borderRadius: "var(--radius-2)",
        fontFamily: "monospace",
      }}
    >
      <Flex align="center" gap="2">
        <Text size="1" style={{ color: statusColor, fontWeight: 600 }}>
          {block.status.tag}
        </Text>
        <Text size="1" style={{ fontWeight: 500 }}>
          {block.tool_name}
        </Text>
        <Text size="1" color="gray" style={{ opacity: 0.7 }}>
          {role.tag}
        </Text>
      </Flex>
      {hasArgs && (
        <>
          <Box
            style={{
              fontSize: "var(--font-size-1)",
              whiteSpace: "pre-wrap",
              overflowX: "auto",
              marginTop: "var(--space-1)",
              color: "var(--gray-11)",
              maxHeight: expanded ? undefined : "8rem",
              overflow: expanded ? "visible" : "hidden",
            }}
          >
            {prettyArgs}
          </Box>
          <button
            type="button"
            onClick={() => setExpanded((value) => !value)}
            style={{
              all: "unset",
              cursor: "pointer",
              fontSize: "var(--font-size-1)",
              color: "var(--gray-10)",
              marginTop: "var(--space-1)",
              display: "block",
            }}
          >
            {expanded ? "▲ collapse" : "▼ expand"}
          </button>
        </>
      )}
      {block.error && (
        <Text size="1" style={{ color: "var(--red-11)", marginTop: "var(--space-1)" }}>
          {block.error.message}
        </Text>
      )}
    </Box>
  );
});

function SyntheticHumanMessage({
  block,
  synthetic,
  isSelected,
  onBubbleClick,
  onReplyRequest,
}: {
  block: TextBlockType;
  synthetic: SyntheticHumanText;
  isSelected: boolean;
  onBubbleClick: (e: React.MouseEvent) => void;
  onReplyRequest?: () => void;
}) {
  if (synthetic.kind === "mateActivitySummary") {
    const summaryBlock = { ...block, text: synthetic.body };
    const cls = `${feedBubble} ${feedBubbleActivitySummary}${isSelected ? ` ${feedBubbleSelected}` : ""}`;
    return (
      <Box className={feedRowAgent}>
        <Box className={feedBubbleCol}>
          <Box className={cls} onClick={onBubbleClick}>
            <TextBlock block={summaryBlock as TextBlockType} />
          </Box>
          {isSelected && <BubbleActionBar text={synthetic.body} onReply={onReplyRequest} />}
        </Box>
      </Box>
    );
  }

  if (synthetic.kind === "systemNotification") {
    return (
      <Box
        className={feedSystemMessage}
        data-testid="synthetic-human-text"
        data-synthetic-kind="system-notification"
      >
        <Text className={feedSystemMessageText}>System notification</Text>
      </Box>
    );
  }

  const bodyBlock = { ...block, text: synthetic.body };
  const cls = `${feedBubble}${isSelected ? ` ${feedBubbleSelected}` : ""}`;
  return (
    <Box
      className={feedSystemMessage}
      data-testid="synthetic-human-text"
      data-synthetic-kind="mate-update"
    >
      <Box className={feedBubbleCol} style={{ width: "min(100%, 32rem)" }}>
        <Text className={feedSystemMessageText}>Mate update</Text>
        <Box className={cls} onClick={onBubbleClick}>
          <TextBlock block={bodyBlock as TextBlockType} />
        </Box>
        {isSelected && <BubbleActionBar text={synthetic.body} onReply={onReplyRequest} />}
      </Box>
    </Box>
  );
}

const SingleBlock = memo(function SingleBlock({
  entry,
  sessionId,
  lastUnresolvedPermBlockId,
  agentForBlock,
  taskCompletedDuration,
  debugMode = false,
  isSelected,
  onSelectBlock,
  onReplyRequest,
}: {
  entry: BlockEntry;
  sessionId: string;
  lastUnresolvedPermBlockId: string | undefined;
  agentForBlock: AgentSnapshot | null;
  taskCompletedDuration: number | null;
  debugMode?: boolean;
  isSelected: boolean;
  onSelectBlock: (id: string | null) => void;
  onReplyRequest?: () => void;
}) {
  const { block, blockId, role } = entry;
  const isCaptain = role.tag === "Captain";

  function handleBubbleClick(event: React.MouseEvent) {
    event.stopPropagation();
    onSelectBlock(isSelected ? null : blockId);
  }

  switch (block.tag) {
    case "Text": {
      const isHuman = block.source.tag === "Human";
      const isThought = block.source.tag === "AgentThought";
      const isAgent = block.source.tag === "AgentMessage";
      const syntheticHuman = isHuman ? parseSyntheticHumanText(block) : null;

      if (syntheticHuman) {
        return (
          <SyntheticHumanMessage
            block={block as TextBlockType}
            synthetic={syntheticHuman}
            isSelected={isSelected}
            onBubbleClick={handleBubbleClick}
            onReplyRequest={onReplyRequest}
          />
        );
      }

      if (isHuman && role.tag === "Captain") {
        const className = `${feedBubble} ${feedBubbleUser}${isSelected ? ` ${feedBubbleSelected}` : ""}`;
        return (
          <Box className={feedRowUser}>
            <Box className={`${feedBubbleCol} ${feedBubbleColUser}`}>
              <Box className={className} onClick={handleBubbleClick}>
                <TextBlock block={block as TextBlockType} />
              </Box>
              {isSelected && <BubbleActionBar text={block.text} onReply={onReplyRequest} />}
            </Box>
          </Box>
        );
      }

      if (block.source.tag === "Steer" && role.tag === "Mate") {
        const className = `${feedBubble} ${feedBubbleSteer}${isSelected ? ` ${feedBubbleSelected}` : ""}`;
        return (
          <Box className={feedRowAgent}>
            <Box className={feedBubbleCol}>
              <Box className={className} onClick={handleBubbleClick}>
                <TextBlock block={block as TextBlockType} />
              </Box>
              {isSelected && <BubbleActionBar text={block.text} speakable onReply={onReplyRequest} />}
            </Box>
          </Box>
        );
      }

      if (isHuman && role.tag === "Mate") {
        const className = `${feedBubble} ${feedBubbleRelay}${isSelected ? ` ${feedBubbleSelected}` : ""}`;
        return (
          <Box className={feedRowAgent}>
            <Box className={feedBubbleCol}>
              <Box className={className} onClick={handleBubbleClick}>
                <TextBlock block={block as TextBlockType} />
              </Box>
              {isSelected && <BubbleActionBar text={block.text} speakable onReply={onReplyRequest} />}
            </Box>
          </Box>
        );
      }

      if (isThought) {
        return null;
      }

      const className = `${feedBubble}${isCaptain ? ` ${feedBubbleCaptain}` : ` ${feedBubbleMate}`}${isSelected ? ` ${feedBubbleSelected}` : ""}`;
      return (
        <Box className={feedRowAgent}>
          <Box className={feedBubbleCol}>
            <Box className={className} onClick={handleBubbleClick}>
              <TextBlock block={block as TextBlockType} />
            </Box>
            {isSelected && <BubbleActionBar text={block.text} speakable={isAgent} onReply={onReplyRequest} />}
          </Box>
        </Box>
      );
    }

    case "ToolCall":
      if (!debugMode) return null;
      return <ToolCallDebugBlock block={block} role={role} />;

    case "Error":
      return <ErrorBlock block={block} agentState={agentForBlock?.state ?? { tag: "Idle" }} />;

    case "Permission": {
      const isActive = blockId === lastUnresolvedPermBlockId;
      const permissionId = isActive ? (block.permission_id ?? null) : null;

      async function resolve(optionId: string) {
        if (!permissionId) return;
        const client = await getShipClient();
        await client.resolvePermission(sessionId, permissionId, optionId);
      }

      return <PermissionBlock block={block} onResolve={permissionId ? resolve : undefined} />;
    }

    case "PlanUpdate":
      return null;

    case "Image":
      if (role.tag === "Captain") {
        return (
          <Box className={feedRowUser}>
            <Box className={`${feedBubbleCol} ${feedBubbleColUser}`}>
              <ImageBlock block={block} className={feedImageUser} />
            </Box>
          </Box>
        );
      }
      return (
        <Box className={feedRowAgent}>
          <Box className={feedBubbleCol}>
            <Box className={`${feedBubble} ${feedBubbleRelay}`}>
              <ImageBlock block={block} />
            </Box>
          </Box>
        </Box>
      );

    case "WorkflowMilestone":
      return <WorkflowMilestoneBlock block={block} />;

    case "TaskRecap":
      return <TaskRecapBlock block={block} duration={taskCompletedDuration} />;
  }
});

// ─── Startup state ────────────────────────────────────────────────────────────

function StartupFeedState({ startupState }: { startupState: SessionStartupState }) {
  const tone = startupState.tag === "Failed" ? "error" : "neutral";
  const title = startupState.tag === "Failed" ? "Session startup failed" : "Session startup";
  let detail = "Waiting to begin startup.";
  if (startupState.tag === "Running" || startupState.tag === "Failed") {
    detail = startupState.message;
  }
  return (
    <Box className={startupFeedItem} data-tone={tone}>
      {startupState.tag !== "Failed" && <Spinner size="1" />}
      <Box className={startupFeedBody}>
        <Text className={feedMessageMeta}>{title}</Text>
        <Text size="2">{detail}</Text>
      </Box>
    </Box>
  );
}

// ─── Live bubbles ─────────────────────────────────────────────────────────────

const ThinkingBubble = memo(function ThinkingBubble({
  sessionId,
  avatarSrc,
  agentName,
  agentKind,
  modelLabel,
  effortLabel,
  lastUtterance,
  thinkingTokens,
  toolsOk,
  toolsFailed,
}: {
  sessionId: string;
  avatarSrc: string;
  agentName: string;
  agentKind: AgentSnapshot["kind"];
  modelLabel: string;
  effortLabel: string | null;
  lastUtterance: string;
  thinkingTokens: number;
  toolsOk: number;
  toolsFailed: number;
}) {
  const [hovered, setHovered] = useState(false);

  return (
    <Box
      className={feedRowAgent}
      style={{ paddingBottom: 0, position: "relative" }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {hovered && (
        <div
          style={{
            position: "absolute",
            bottom: "100%",
            left: 0,
            maxWidth: 480,
            marginBottom: 6,
            padding: "var(--space-2) var(--space-3)",
            background: "var(--color-panel-solid)",
            border: "1px solid var(--gray-a4)",
            borderRadius: "16px",
            boxShadow: "0 1px 4px var(--black-a2)",
            zIndex: 10,
            maxHeight: 200,
            overflow: "hidden",
          }}
        >
          <Flex align="center" gap="2" mb={lastUtterance ? "2" : "0"}>
            <AgentKindIcon kind={agentKind} />
            <Text size="2" weight="bold">
              {agentName}
            </Text>
            <Text size="1" color="gray">
              {modelLabel}
            </Text>
            {effortLabel && (
              <Text size="1" color="gray">
                ({effortLabel})
              </Text>
            )}
          </Flex>
          {lastUtterance && (
            <Text
              size="2"
              style={{
                color: "var(--gray-11)",
                whiteSpace: "pre-wrap",
                display: "-webkit-box",
                WebkitLineClamp: 8,
                WebkitBoxOrient: "vertical",
                overflow: "hidden",
              }}
            >
              {lastUtterance}
            </Text>
          )}
        </div>
      )}
      <div className={thinkingBubble}>
        <button
          type="button"
          className={thinkingAvatarBtn}
          onClick={() => {
            void (async () => {
              const client = await getShipClient();
              await client.stopAgents(sessionId);
            })();
          }}
          title="Stop agent"
        >
          <img src={avatarSrc} alt={agentName} className={thinkingAvatarImg} />
          <span className={thinkingAvatarStop}>
            <Stop size={14} weight="fill" />
          </span>
        </button>
        {toolsOk > 0 && (
          <Text size="2" style={{ color: "var(--green-11)" }}>
            {toolsOk}✓
          </Text>
        )}
        {toolsFailed > 0 && (
          <Text size="2" style={{ color: "var(--red-11)" }}>
            {toolsFailed}✗
          </Text>
        )}
        <Text size="2" className={shimmerText} style={{ marginLeft: "auto" }}>
          {thinkingTokens} tokens
        </Text>
      </div>
    </Box>
  );
});

// ─── Main component ───────────────────────────────────────────────────────────

interface Props {
  sessionId: string;
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
  blocks: BlockEntry[];
  startupState: SessionStartupState | null;
  taskCompletedDuration: number | null;
  captainTurnStartedAt?: string | null;
  mateTurnStartedAt?: string | null;
  userAvatarUrl?: string | null;
  loading?: boolean;
  loadingLabel?: string;
  debugMode?: boolean;
  onImageDrop?: (files: File[]) => void;
  onImageDragStateChange?: (isDragOver: boolean) => void;
  onReplyRequest?: () => void;
}

type SessionFeedAnimationBaseline = {
  blockIds: Set<string>;
  established: boolean;
};

const sessionFeedAnimationBaselines = new Map<string, SessionFeedAnimationBaseline>();

function getSessionFeedAnimationBaseline(sessionId: string) {
  let baseline = sessionFeedAnimationBaselines.get(sessionId);
  if (!baseline) {
    baseline = { blockIds: new Set(), established: false };
    sessionFeedAnimationBaselines.set(sessionId, baseline);
  }
  return baseline;
}

export function resetUnifiedFeedAnimationBaselinesForTest() {
  sessionFeedAnimationBaselines.clear();
}

// r[ui.event-stream.grouping]
// r[view.agent-panel.state]
export function UnifiedFeed({
  sessionId,
  captain,
  mate,
  blocks,
  startupState,
  taskCompletedDuration,
  captainTurnStartedAt = null,
  mateTurnStartedAt = null,
  loading,
  loadingLabel,
  debugMode = false,
  onImageDrop,
  onImageDragStateChange,
  onReplyRequest,
}: Props) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickyScroll = useRef(true);
  const [atBottom, setAtBottom] = useState(true);
  const [selectedBlockId, setSelectedBlockId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<HTMLDivElement | null>(null);
  const isImageDragOver = useDocumentDrop(dropTarget, onImageDrop ?? NOOP_IMAGE_DROP);
  const [tick, setTick] = useState(() => Date.now());

  useEffect(() => {
    const id = setInterval(() => setTick(Date.now()), 30_000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    onImageDragStateChange?.(isImageDragOver);
  }, [isImageDragOver, onImageDragStateChange]);

  useEffect(() => {
    return () => {
      onImageDragStateChange?.(false);
    };
  }, [onImageDragStateChange]);

  const sessionAnimationBaseline = getSessionFeedAnimationBaseline(sessionId);
  if (loading || !sessionAnimationBaseline.established) {
    for (const block of blocks) {
      sessionAnimationBaseline.blockIds.add(block.blockId);
    }
  }
  if (!loading) {
    sessionAnimationBaseline.established = true;
  }

  const humanMsgCount = useMemo(
    () => blocks.filter((block) => block.block.tag === "Text" && block.block.source.tag === "Human").length,
    [blocks],
  );

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    element.scrollTop = element.scrollHeight;
    stickyScroll.current = true;
    setAtBottom(true);
  }, [humanMsgCount]);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element || !stickyScroll.current) return;
    element.scrollTop = element.scrollHeight;
  }, [blocks, captain?.state, mate?.state]);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    const observer = new ResizeObserver(() => {
      if (stickyScroll.current) {
        element.scrollTop = element.scrollHeight;
      }
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  function handleScroll() {
    const element = scrollRef.current;
    if (!element) return;
    const nextAtBottom = element.scrollHeight - element.scrollTop - element.clientHeight < 32;
    stickyScroll.current = nextAtBottom;
    setAtBottom(nextAtBottom);
  }

  function scrollToBottom() {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }

  const showStartupFeed = startupState?.tag !== "Ready";
  const captainWorking = captain?.state.tag === "Working";
  const mateWorking = mate?.state.tag === "Working";
  const truncated = blocks.length > MAX_RENDERED_BLOCKS;
  const visibleBlocks = useMemo(
    () => (truncated ? blocks.slice(blocks.length - MAX_RENDERED_BLOCKS) : blocks),
    [blocks, truncated],
  );
  const segments = useMemo(() => buildSegments(visibleBlocks, debugMode), [visibleBlocks, debugMode]);
  const lastUnresolvedPermBlockId = useMemo(() => {
    let blockId: string | undefined;
    for (const entry of visibleBlocks) {
      if (entry.block.tag === "Permission" && !entry.block.resolution) {
        blockId = entry.blockId;
      }
    }
    return blockId;
  }, [visibleBlocks]);
  const captainTurn = useMemo(
    () => (captainWorking ? computeTurnStats(blocks, "Captain", captainTurnStartedAt) : null),
    [captainWorking, blocks, captainTurnStartedAt],
  );
  const mateTurn = useMemo(
    () => (mateWorking ? computeTurnStats(blocks, "Mate", mateTurnStartedAt) : null),
    [mateWorking, blocks, mateTurnStartedAt],
  );
  const renderedSegments = useMemo<RenderedSegment[]>(() => {
    let previousTimestampMs: number | null = null;

    return segments.map((seg) => {
      const agentRole = segmentAgentRole(seg);
      const agentForBlock =
        agentRole?.tag === "Captain"
          ? captain
          : agentRole?.tag === "Mate"
            ? mate
            : seg.entry.role.tag === "Captain"
              ? captain
              : mate;
      const timestampMs = parseTimestampMs(seg.entry.timestamp);
      const gapLabel =
        previousTimestampMs != null &&
        timestampMs != null &&
        timestampMs - previousTimestampMs > GAP_MS
          ? formatRelativeTime(timestampMs, tick)
          : null;

      if (timestampMs != null) {
        previousTimestampMs = timestampMs;
      }

      return {
        seg,
        agentForBlock,
        isTaskRecap: seg.entry.block.tag === "TaskRecap",
        gapLabel,
        timestampMs,
      };
    });
  }, [segments, captain, mate, tick]);
  const trailingGapLabel = useMemo(() => {
    const lastTimestampMs = renderedSegments[renderedSegments.length - 1]?.timestampMs ?? null;
    if (lastTimestampMs == null || tick - lastTimestampMs <= GAP_MS) return null;
    return formatRelativeTime(lastTimestampMs, tick);
  }, [renderedSegments, tick]);


  return (
    <Box ref={setDropTarget} className={unifiedFeedRoot} data-testid="session-feed-drop-target">
      {loading && (
        <Flex align="center" gap="2" px="3" py="2" style={{ flexShrink: 0 }}>
          <Spinner size="1" />
          <Text size="1" color="gray">
            {loadingLabel ?? "Replaying events…"}
          </Text>
        </Flex>
      )}

      <Box
        ref={scrollRef}
        className={unifiedFeedScroll}
        onScroll={handleScroll}
        onClick={() => setSelectedBlockId(null)}
      >
        {!atBottom && (
          <button
            type="button"
            className={scrollToBottomBtn}
            onClick={scrollToBottom}
            aria-label="Scroll to bottom"
          >
            <ArrowDown size={16} />
          </button>
        )}
        <Box className={unifiedFeedStream}>
          {showStartupFeed && startupState && (
            <Box className={feedContentColumn}>
              <StartupFeedState startupState={startupState} />
            </Box>
          )}

          {truncated && (
            <Flex align="center" justify="center" py="2">
              <Text size="1" color="gray">
                Showing last {MAX_RENDERED_BLOCKS} of {blocks.length} blocks
              </Text>
            </Flex>
          )}

          {renderedSegments.map(({ seg, agentForBlock, gapLabel, isTaskRecap }) => {
            const alreadyKnown = sessionAnimationBaseline.blockIds.has(seg.entry.blockId);
            const animate = !loading && sessionAnimationBaseline.established && !alreadyKnown;
            if (!alreadyKnown) {
              sessionAnimationBaseline.blockIds.add(seg.entry.blockId);
            }

            const blockContent = animate ? (
              <div className={feedRowAnimate}>
                <SingleBlock
                  entry={seg.entry}
                  sessionId={sessionId}
                  lastUnresolvedPermBlockId={lastUnresolvedPermBlockId}
                  agentForBlock={agentForBlock}
                  taskCompletedDuration={taskCompletedDuration}
                  debugMode={debugMode}
                  isSelected={selectedBlockId === seg.entry.blockId}
                  onSelectBlock={setSelectedBlockId}
                  onReplyRequest={onReplyRequest}
                />
              </div>
            ) : (
              <SingleBlock
                entry={seg.entry}
                sessionId={sessionId}
                lastUnresolvedPermBlockId={lastUnresolvedPermBlockId}
                agentForBlock={agentForBlock}
                taskCompletedDuration={taskCompletedDuration}
                debugMode={debugMode}
                isSelected={selectedBlockId === seg.entry.blockId}
                onSelectBlock={setSelectedBlockId}
                onReplyRequest={onReplyRequest}
              />
            );

            return (
              <Fragment key={seg.entry.blockId}>
                {gapLabel && (
                  <Box className={feedContentColumn}>
                    <Text className={feedTimeGap}>{gapLabel}</Text>
                  </Box>
                )}
                {isTaskRecap ? blockContent : <Box className={feedContentColumn}>{blockContent}</Box>}
                {debugMode && (
                  <Box className={feedContentColumn}>
                    <RawBlockDebug entry={seg.entry} />
                  </Box>
                )}
              </Fragment>
            );
          })}
          {trailingGapLabel && (
            <Box className={feedContentColumn}>
              <Text className={feedTimeGap}>{trailingGapLabel}</Text>
            </Box>
          )}
        </Box>

        <Box className={feedContentColumn}>
          <Box className={liveBubblesRow}>
            <div
              className={liveBubbleSlot}
              style={{
                opacity: captainTurn && captain ? 1 : 0,
                pointerEvents: captainTurn && captain ? "auto" : "none",
              }}
            >
              {captainTurn && captain && (
                <ThinkingBubble
                  sessionId={sessionId}
                  avatarSrc={captainAvatar}
                  agentName="Captain"
                  agentKind={captain.kind}
                  modelLabel={captain.model_id ?? "unknown"}
                  effortLabel={captain.effort_value_id}
                  lastUtterance={captainTurn.lastUtterance}
                  thinkingTokens={captainTurn.tokens}
                  toolsOk={captainTurn.ok}
                  toolsFailed={captainTurn.failed}
                />
              )}
            </div>
            <div
              className={liveBubbleSlot}
              style={{
                opacity: mateTurn && mate ? 1 : 0,
                pointerEvents: mateTurn && mate ? "auto" : "none",
              }}
            >
              {mateTurn && mate && (
                <ThinkingBubble
                  sessionId={sessionId}
                  avatarSrc={mateAvatar}
                  agentName="Mate"
                  agentKind={mate.kind}
                  modelLabel={mate.model_id ?? "unknown"}
                  effortLabel={mate.effort_value_id}
                  lastUtterance={mateTurn.lastUtterance}
                  thinkingTokens={mateTurn.tokens}
                  toolsOk={mateTurn.ok}
                  toolsFailed={mateTurn.failed}
                />
              )}
            </div>
          </Box>
        </Box>
      </Box>
    </Box>
  );
}
