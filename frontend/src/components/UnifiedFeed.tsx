import { Fragment, useState, useRef, useEffect } from "react";
import { Box, Flex, Spinner, Text } from "@radix-ui/themes";
import { ArrowDown, CaretRight, Stop } from "@phosphor-icons/react";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import type { AgentSnapshot, ContentBlock, Role, SessionStartupState } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { BubbleActions, TextBlock } from "./blocks/TextBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import { ImageBlock } from "./blocks/ImageBlock";
import { getShipClient } from "../api/client";
import { encode } from "gpt-tokenizer";

const tokenCache = new Map<string, number>();

function countTokens(text: string): number {
  const cached = tokenCache.get(text);
  if (cached !== undefined) return cached;
  const count = encode(text).length;
  tokenCache.set(text, count);
  return count;
}

import {
  feedBubble,
  feedBubbleCol,
  feedBubbleColUser,
  feedBubbleCaptain,
  feedBubbleMate,
  feedBubbleRelay,
  feedBubbleUser,
  feedBubbleActivitySummary,
  feedTimestamp,
  feedRowAgent,
  feedRowUser,
  feedSystemMessage,
  liveBubbleDot,
  liveBubblesRow,
  thinkingBubble,
  shimmerText,
  thinkingStopBtn,
  startupFeedBody,
  startupFeedItem,
  feedMessageMeta,
  scrollToBottomBtn,
  unifiedFeedRoot,
  unifiedFeedScroll,
  unifiedFeedStream,
  feedContentColumn,
  userAvatar,
  userAvatarSpacer,
  feedBubbleWithActions,
  feedBubbleWithActionsUser,
  feedImageUser,
  diffAdd,
  diffRemove,
  diffContext,
  taskRecapBoundary,
  taskRecapHeader,
  taskRecapEyebrow,
  taskRecapTitle,
  taskRecapSummary,
  taskRecapCommitList,
  taskRecapCommitRow,
  taskRecapCommitToggle,
  taskRecapCommitStatic,
  taskRecapCommitHash,
  taskRecapCommitSubject,
  taskRecapCaret,
  taskRecapDiff,
  taskRecapDiffInner,
} from "../styles/session-view.css";

type TextBlockType = Extract<ContentBlock, { tag: "Text" }>;

function formatTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

// ─── System message detection ─────────────────────────────────────────────────
// Human blocks injected by the server (task assignments, system prompts) are
// very long and contain instructions. We collapse them to a short label.

function isMateActivitySummary(block: Extract<ContentBlock, { tag: "Text" }>): boolean {
  if (block.source.tag !== "Human") return false;
  return block.text.includes("<mate-activity-summary>");
}

function extractMateActivitySummary(text: string): string {
  const match = text.match(/<mate-activity-summary>\n?([\s\S]*?)\n?<\/mate-activity-summary>/);
  return match ? match[1].trim() : text;
}

function isSystemInjection(block: Extract<ContentBlock, { tag: "Text" }>): boolean {
  if (block.source.tag !== "Human") return false;
  if (isMateActivitySummary(block)) return false;
  return block.text.includes("<system-notification>");
}
function formatDuration(totalSeconds: number): string {
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const mins = Math.floor(totalSeconds / 60);
  const secs = totalSeconds % 60;
  if (secs === 0) return `${mins}m`;
  return `${mins}m ${secs}s`;
}

// ─── Feed segmentation ────────────────────────────────────────────────────────

type SingleSegment = { kind: "single"; entry: BlockEntry };
type FeedSegment = SingleSegment;

function buildSegments(blocks: BlockEntry[], debugMode: boolean): FeedSegment[] {
  const visible = blocks.filter(
    (b) =>
      b.block.tag !== "PlanUpdate" &&
      (debugMode || b.block.tag !== "ToolCall") &&
      !(b.block.tag === "Text" && b.block.source.tag === "AgentThought") &&
      b.role.tag !== "Mate",
  );
  return visible.map((entry) => ({ kind: "single", entry }));
}

// Returns the "agent side" role of a segment, or null if it's a real user message.
function segmentAgentRole(seg: FeedSegment): Role | null {
  const { block, role } = seg.entry;
  if (block.tag === "Text" && block.source.tag === "Human" && !isSystemInjection(block)) {
    if (role.tag === "Captain") return null; // real user message → right side
    return { tag: "Captain" }; // captain relaying to mate → left side
  }
  if (block.tag === "Image" && role.tag === "Captain") {
    return null; // user-sent image → right side
  }
  if (block.tag === "TaskRecap") return null; // system notice, no avatar
  return role;
}

// ─── User avatar ──────────────────────────────────────────────────────────────

function UserAvatar({ url }: { url: string | null }) {
  if (!url) return <div className={userAvatarSpacer} />;
  return <img src={url} className={userAvatar} alt="You" />;
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
          if (line.startsWith("+") && !line.startsWith("+++"))
            return (
              <span key={index} className={diffAdd}>
                {line}
              </span>
            );
          if (line.startsWith("-") && !line.startsWith("---"))
            return (
              <span key={index} className={diffRemove}>
                {line}
              </span>
            );
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

type TaskRecapBlockType = Extract<ContentBlock, { tag: "TaskRecap" }>;

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
      className={taskRecapBoundary}
      data-testid="task-recap-boundary"
      data-feed-boundary="phase-break"
    >
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
          {commits.map((c) => {
            const expanded = expandedHashes.has(c.hash);
            const commitLine = (
              <>
                {c.diff && (
                  <CaretRight
                    size={12}
                    className={taskRecapCaret}
                    style={{ transform: expanded ? "rotate(90deg)" : "rotate(0deg)" }}
                  />
                )}
                <Text className={taskRecapCommitHash}>{c.hash}</Text>
                <Text className={taskRecapCommitSubject}>{c.subject}</Text>
              </>
            );

            return (
              <Box key={c.hash} className={taskRecapCommitRow}>
                {c.diff ? (
                  <button
                    type="button"
                    className={taskRecapCommitToggle}
                    onClick={() => toggleExpanded(c.hash)}
                  >
                    {commitLine}
                  </button>
                ) : (
                  <Flex className={taskRecapCommitStatic} align="center" gap="2">
                    {commitLine}
                  </Flex>
                )}
                {expanded && c.diff && <CommitDiffView diff={c.diff} />}
              </Box>
            );
          })}
        </Box>
      )}
    </Box>
  );
}

// ─── Raw block debug ──────────────────────────────────────────────────────────

function RawBlockDebug({ entry }: { entry: BlockEntry }) {
  const [expanded, setExpanded] = useState(false);
  const json = JSON.stringify(
    { blockId: entry.blockId, role: entry.role, block: entry.block },
    null,
    2,
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
        onClick={() => setExpanded((v) => !v)}
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

type ToolCallBlockType = Extract<ContentBlock, { tag: "ToolCall" }>;

function ToolCallDebugBlock({ block, role }: { block: ToolCallBlockType; role: Role }) {
  const [expanded, setExpanded] = useState(false);
  const statusColor =
    block.status.tag === "Success"
      ? "var(--green-9)"
      : block.status.tag === "Failure"
        ? "var(--red-9)"
        : "var(--gray-9)";
  const prettyArgs = (() => {
    try {
      return JSON.stringify(JSON.parse(block.arguments), null, 2);
    } catch {
      return block.arguments;
    }
  })();
  const hasArgs = block.arguments && block.arguments !== "{}";
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
            onClick={() => setExpanded((v) => !v)}
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
}

function SingleBlock({
  entry,
  sessionId,
  lastUnresolvedPermBlockId,
  agentForBlock,
  isLast,
  userAvatarUrl,
  taskCompletedDuration,
  debugMode = false,
}: {
  entry: BlockEntry;
  sessionId: string;
  lastUnresolvedPermBlockId: string | undefined;
  agentForBlock: AgentSnapshot | null;
  isLast: boolean;
  userAvatarUrl: string | null;
  taskCompletedDuration: number | null;
  debugMode?: boolean;
}) {
  const { block, blockId, role } = entry;
  const isCaptain = role.tag === "Captain";

  switch (block.tag) {
    case "Text": {
      if (block.text.trim() === "") return null;

      const isHuman = block.source.tag === "Human";
      const isThought = block.source.tag === "AgentThought";

      // Server-injected system message — hide
      if (isHuman && isSystemInjection(block)) {
        return null;
      }

      // Mate activity summary injected by Haiku — yellow bubble
      if (isHuman && isMateActivitySummary(block)) {
        const summary = extractMateActivitySummary(block.text);
        const summaryBlock = { ...block, text: summary };
        return (
          <Box className={feedRowAgent}>
            <Box className={feedBubbleWithActions}>
              <Box className={feedBubbleCol}>
                <Box className={`${feedBubble} ${feedBubbleActivitySummary}`}>
                  <TextBlock block={summaryBlock as TextBlockType} />
                </Box>
              </Box>
              <BubbleActions
                block={summaryBlock as TextBlockType}
                speakable={false}
                isLast={isLast}
                timestamp={entry.timestamp ?? undefined}
              />
            </Box>
          </Box>
        );
      }

      // Real user message — right side
      if (isHuman && role.tag === "Captain") {
        return (
          <Box className={feedRowUser}>
            <Box className={`${feedBubbleWithActions} ${feedBubbleWithActionsUser}`}>
              <Box className={`${feedBubbleCol} ${feedBubbleColUser}`}>
                <Box className={`${feedBubble} ${feedBubbleUser}`}>
                  <TextBlock block={block as TextBlockType} />
                </Box>
              </Box>
            </Box>
          </Box>
        );
      }

      // Captain relaying to mate — left side, amber tint
      if (isHuman && role.tag === "Mate") {
        return (
          <Box className={feedRowAgent}>
            <Box className={feedBubbleWithActions}>
              <Box className={feedBubbleCol}>
                <Box className={`${feedBubble} ${feedBubbleRelay}`}>
                  <TextBlock block={block as TextBlockType} />
                </Box>
              </Box>
              <BubbleActions
                block={block as TextBlockType}
                speakable
                isLast={isLast}
                timestamp={entry.timestamp ?? undefined}
              />
            </Box>
          </Box>
        );
      }

      // Thought block — hidden
      if (isThought) {
        return null;
      }

      // Agent message — left side
      return (
        <Box className={feedRowAgent}>
          <Box className={feedBubbleWithActions}>
            <Box className={feedBubbleCol}>
              <Box
                className={`${feedBubble}${isCaptain ? ` ${feedBubbleCaptain}` : ` ${feedBubbleMate}`}`}
              >
                <TextBlock block={block as TextBlockType} />
              </Box>
            </Box>
            <BubbleActions
              block={block as TextBlockType}
              speakable
              isLast={isLast}
              timestamp={entry.timestamp ?? undefined}
            />
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
      if (block.resolution?.tag === "Approved") return null;
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

    case "Image": {
      // User-sent image (captain role, human source) — right side
      if (role.tag === "Captain") {
        return (
          <Box className={feedRowUser}>
            <Box className={`${feedBubbleCol} ${feedBubbleColUser}`}>
              <ImageBlock block={block} className={feedImageUser} />
            </Box>
          </Box>
        );
      }
      // Relay image — left side
      return (
        <Box className={feedRowAgent}>
          <Box className={feedBubbleCol}>
            <Box className={`${feedBubble} ${feedBubbleRelay}`}>
              <ImageBlock block={block} />
            </Box>
            {isLast && entry.timestamp && (
              <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
            )}
          </Box>
        </Box>
      );
    }

    case "TaskRecap":
      return <TaskRecapBlock block={block} duration={taskCompletedDuration} />;
  }
}

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

function ThinkingBubble({
  sessionId,
  avatarSrc,
  agentName,
  thinkingTokens,
  toolsOk,
  toolsFailed,
}: {
  sessionId: string;
  avatarSrc: string;
  agentName: string;
  thinkingTokens: number;
  toolsOk: number;
  toolsFailed: number;
}) {
  return (
    <Box className={feedRowAgent} style={{ paddingBottom: 0 }}>
      <div className={thinkingBubble}>
        <button
          type="button"
          className={thinkingStopBtn}
          onClick={() => {
            void (async () => {
              const client = await getShipClient();
              await client.stopAgents(sessionId);
            })();
          }}
          title="Stop agent"
        >
          <Stop size={14} weight="fill" />
        </button>
        <img
          src={avatarSrc}
          alt={agentName}
          style={{ width: 36, height: 36, borderRadius: "50%", flexShrink: 0 }}
        />
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
}

// ─── Main component ───────────────────────────────────────────────────────────

interface Props {
  sessionId: string;
  captain: AgentSnapshot | null;
  mate: AgentSnapshot | null;
  blocks: BlockEntry[];
  startupState: SessionStartupState | null;
  taskCompletedDuration: number | null;
  userAvatarUrl?: string | null;
  loading?: boolean;
  loadingLabel?: string;
  debugMode?: boolean;
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
  userAvatarUrl = null,
  loading,
  loadingLabel,
  debugMode = false,
}: Props) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickyScroll = useRef(true);
  const [atBottom, setAtBottom] = useState(true);

  const humanMsgCount = blocks.filter(
    (b) => b.block.tag === "Text" && b.block.source.tag === "Human",
  ).length;

  // Always scroll to bottom when the user sends a message.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    stickyScroll.current = true;
    setAtBottom(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [humanMsgCount]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el || !stickyScroll.current) return;
    el.scrollTop = el.scrollHeight;
  }, [blocks, captain?.state, mate?.state]);

  function handleScroll() {
    const el = scrollRef.current;
    if (!el) return;
    const isAtBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 32;
    stickyScroll.current = isAtBottom;
    setAtBottom(isAtBottom);
  }

  function scrollToBottom() {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }

  const showStartupFeed = startupState?.tag !== "Ready";
  const captainWorking = captain?.state.tag === "Working";
  const mateWorking = mate?.state.tag === "Working";

  const MAX_RENDERED_BLOCKS = 80;
  const truncated = blocks.length > MAX_RENDERED_BLOCKS;
  const visibleBlocks = truncated ? blocks.slice(blocks.length - MAX_RENDERED_BLOCKS) : blocks;

  let thinkingTokens = 0;
  let toolsOk = 0;
  let toolsFailed = 0;
  if (captainWorking) {
    let lastCaptainMsgIdx = -1;
    for (let i = visibleBlocks.length - 1; i >= 0; i--) {
      const b = visibleBlocks[i];
      if (
        b.role.tag === "Captain" &&
        b.block.tag === "Text" &&
        b.block.source.tag === "AgentMessage"
      ) {
        lastCaptainMsgIdx = i;
        break;
      }
    }
    const turnBlocks = visibleBlocks.slice(lastCaptainMsgIdx + 1);
    for (const b of turnBlocks) {
      if (b.role.tag !== "Captain") continue;
      if (b.block.tag === "Text" && b.block.source.tag === "AgentThought") {
        thinkingTokens += countTokens(b.block.text);
      } else if (b.block.tag === "ToolCall") {
        if (b.block.status.tag === "Success") toolsOk++;
        else if (b.block.status.tag === "Failure") toolsFailed++;
        thinkingTokens += countTokens(b.block.arguments);
        if (b.block.raw_output != null) {
          thinkingTokens += countTokens(JSON.stringify(b.block.raw_output));
        }
      }
    }
  }

  let mateThinkingTokens = 0;
  let mateToolsOk = 0;
  let mateToolsFailed = 0;
  if (mateWorking) {
    let lastMateMsgIdx = -1;
    for (let i = visibleBlocks.length - 1; i >= 0; i--) {
      const b = visibleBlocks[i];
      if (
        b.role.tag === "Mate" &&
        b.block.tag === "Text" &&
        b.block.source.tag === "AgentMessage"
      ) {
        lastMateMsgIdx = i;
        break;
      }
    }
    const mateTurnBlocks = visibleBlocks.slice(lastMateMsgIdx + 1);
    for (const b of mateTurnBlocks) {
      if (b.role.tag !== "Mate") continue;
      if (b.block.tag === "Text" && b.block.source.tag === "AgentThought") {
        mateThinkingTokens += countTokens(b.block.text);
      } else if (b.block.tag === "ToolCall") {
        if (b.block.status.tag === "Success") mateToolsOk++;
        else if (b.block.status.tag === "Failure") mateToolsFailed++;
        mateThinkingTokens += countTokens(b.block.arguments);
        if (b.block.raw_output != null) {
          mateThinkingTokens += countTokens(JSON.stringify(b.block.raw_output));
        }
      }
    }
  }

  let lastUnresolvedPermBlockId: string | undefined;
  for (const entry of visibleBlocks) {
    if (entry.block.tag === "Permission" && !entry.block.resolution) {
      lastUnresolvedPermBlockId = entry.blockId;
    }
  }

  const segments = buildSegments(visibleBlocks, debugMode);

  return (
    <Box className={unifiedFeedRoot}>
      {loading && (
        <Flex align="center" gap="2" px="3" py="2" style={{ flexShrink: 0 }}>
          <Spinner size="1" />
          <Text size="1" color="gray">
            {loadingLabel ?? "Replaying events…"}
          </Text>
        </Flex>
      )}

      <Box ref={scrollRef} className={unifiedFeedScroll} onScroll={handleScroll}>
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
        <Box className={feedContentColumn}>
          <Box className={unifiedFeedStream}>
            {showStartupFeed && startupState && <StartupFeedState startupState={startupState} />}

            {truncated && (
              <Flex align="center" justify="center" py="2">
                <Text size="1" color="gray">
                  Showing last {MAX_RENDERED_BLOCKS} of {blocks.length} blocks
                </Text>
              </Flex>
            )}

            {segments.map((seg, idx) => {
              const agentRole = segmentAgentRole(seg);
              const agentForBlock =
                agentRole?.tag === "Captain"
                  ? captain
                  : agentRole?.tag === "Mate"
                    ? mate
                    : seg.entry.role.tag === "Captain"
                      ? captain
                      : mate;
              return (
                <Fragment key={seg.entry.blockId}>
                  <SingleBlock
                    entry={seg.entry}
                    sessionId={sessionId}
                    lastUnresolvedPermBlockId={lastUnresolvedPermBlockId}
                    agentForBlock={agentForBlock}
                    isLast={idx === segments.length - 1}
                    userAvatarUrl={userAvatarUrl}
                    taskCompletedDuration={taskCompletedDuration}
                    debugMode={debugMode}
                  />
                  {debugMode && <RawBlockDebug entry={seg.entry} />}
                </Fragment>
              );
            })}
          </Box>

          <Box className={liveBubblesRow}>
            {captainWorking && (
              <ThinkingBubble
                sessionId={sessionId}
                avatarSrc={captainAvatar}
                agentName="Captain"
                thinkingTokens={thinkingTokens}
                toolsOk={toolsOk}
                toolsFailed={toolsFailed}
              />
            )}
            {mateWorking && (
              <ThinkingBubble
                sessionId={sessionId}
                avatarSrc={mateAvatar}
                agentName="Mate"
                thinkingTokens={mateThinkingTokens}
                toolsOk={mateToolsOk}
                toolsFailed={mateToolsFailed}
              />
            )}
          </Box>
        </Box>
      </Box>
    </Box>
  );
}
