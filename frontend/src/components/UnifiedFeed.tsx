import { Fragment, useState, useRef, useEffect } from "react";
import { Box, Flex, ScrollArea, Spinner, Text } from "@radix-ui/themes";
import { ArrowDown, CaretRight } from "@phosphor-icons/react";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import type { AgentSnapshot, ContentBlock, Role, SessionStartupState } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { BubbleActions, TextBlock } from "./blocks/TextBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import { ImageBlock } from "./blocks/ImageBlock";
import { getShipClient } from "../api/client";
import {
  feedBubble,
  feedBubbleCol,
  feedBubbleColUser,
  feedBubbleCaptain,
  feedBubbleMate,
  feedBubbleRelay,
  feedBubbleUser,
  feedTimestamp,
  feedRowAgent,
  feedRowUser,
  feedSystemMessage,
  liveBubble,
  liveBubbleDot,
  liveBubblesRow,
  thinkingBubbleOuter,
  thinkingBubbleInner,
  startupFeedBody,
  startupFeedItem,
  feedMessageMeta,
  scrollToBottomBtn,
  unifiedFeedRoot,
  unifiedFeedScroll,
  unifiedFeedStream,
  userAvatar,
  userAvatarSpacer,
  feedBubbleWithActions,
  diffAdd,
  diffRemove,
  diffContext,
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

function isSystemInjection(block: Extract<ContentBlock, { tag: "Text" }>): boolean {
  if (block.source.tag !== "Human") return false;
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

function buildSegments(blocks: BlockEntry[]): FeedSegment[] {
  const visible = blocks.filter(
    (b) =>
      b.block.tag !== "PlanUpdate" &&
      b.block.tag !== "ToolCall" &&
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
    <ScrollArea
      style={{
        maxHeight: "16rem",
        width: "100%",
        overflowX: "auto",
        marginTop: "var(--space-1)",
      }}
    >
      <Box
        style={{
          fontFamily: "var(--font-mono, monospace)",
          fontSize: "var(--font-size-1)",
          whiteSpace: "pre",
          textAlign: "left",
          minWidth: "max-content",
        }}
      >
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
    </ScrollArea>
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
  const [expandedHash, setExpandedHash] = useState<string | null>(null);
  const { commits, stats } = block;

  return (
    <Box
      className={feedSystemMessage}
      style={{
        flexDirection: "column",
        alignItems: "center",
        gap: "var(--space-1)",
        paddingTop: "var(--space-2)",
        paddingBottom: "var(--space-2)",
      }}
    >
      <Text style={{ fontSize: "var(--font-size-1)", color: "var(--gray-9)", fontWeight: 500 }}>
        Work accepted
      </Text>
      {duration != null && (
        <Text style={{ fontSize: "var(--font-size-1)", color: "var(--gray-9)" }}>
          completed in {formatDuration(duration)}
        </Text>
      )}
      {commits.length > 0 && (
        <Box
          style={{
            fontFamily: "var(--font-mono, monospace)",
            fontSize: "var(--font-size-1)",
            color: "var(--gray-10)",
            textAlign: "center",
            width: "100%",
            minWidth: 0,
          }}
        >
          {commits.map((c) => (
            <Box key={c.hash}>
              <Box
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: "var(--space-1)",
                  cursor: c.diff ? "pointer" : undefined,
                }}
                onClick={() => c.diff && setExpandedHash(expandedHash === c.hash ? null : c.hash)}
              >
                {c.diff && (
                  <CaretRight
                    size={10}
                    style={{
                      color: "var(--gray-8)",
                      transition: "transform 0.15s ease",
                      transform: expandedHash === c.hash ? "rotate(90deg)" : "rotate(0deg)",
                      flexShrink: 0,
                    }}
                  />
                )}
                <Text style={{ color: "var(--gray-8)" }}>{c.hash}</Text> <Text>{c.subject}</Text>
              </Box>
              {expandedHash === c.hash && c.diff && <CommitDiffView diff={c.diff} />}
            </Box>
          ))}
        </Box>
      )}
      {stats && (
        <Text
          style={{
            fontSize: "var(--font-size-1)",
            color: "var(--gray-9)",
            fontFamily: "var(--font-mono, monospace)",
          }}
        >
          <span style={{ color: "var(--green-9)" }}>+{stats.insertions}</span>{" "}
          <span style={{ color: "var(--red-9)" }}>−{stats.deletions}</span> across{" "}
          {stats.files_changed} file{stats.files_changed !== 1 ? "s" : ""}
        </Text>
      )}
    </Box>
  );
}

// ─── Single block ─────────────────────────────────────────────────────────────

function SingleBlock({
  entry,
  sessionId,
  lastUnresolvedPermBlockId,
  agentForBlock,
  isLast,
  userAvatarUrl,
  taskCompletedDuration,
}: {
  entry: BlockEntry;
  sessionId: string;
  lastUnresolvedPermBlockId: string | undefined;
  agentForBlock: AgentSnapshot | null;
  isLast: boolean;
  userAvatarUrl: string | null;
  taskCompletedDuration: number | null;
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

      // Real user message — right side
      if (isHuman && role.tag === "Captain") {
        return (
          <Box className={feedRowUser}>
            <Box className={feedBubbleWithActions}>
              <BubbleActions block={block as TextBlockType} />
              <Box className={`${feedBubbleCol} ${feedBubbleColUser}`}>
                <Box className={`${feedBubble} ${feedBubbleUser}`}>
                  <TextBlock block={block as TextBlockType} />
                </Box>
                {isLast && entry.timestamp && (
                  <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
                )}
              </Box>
            </Box>
            <UserAvatar url={userAvatarUrl} />
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
                {isLast && entry.timestamp && (
                  <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
                )}
              </Box>
              <BubbleActions block={block as TextBlockType} speakable />
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
              {isLast && entry.timestamp && (
                <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
              )}
            </Box>
            <BubbleActions block={block as TextBlockType} speakable />
          </Box>
        </Box>
      );
    }

    case "ToolCall":
      return null;

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
              <Box className={`${feedBubble} ${feedBubbleUser}`}>
                <ImageBlock block={block} />
              </Box>
              {isLast && entry.timestamp && (
                <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
              )}
            </Box>
            <UserAvatar url={userAvatarUrl} />
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

function LiveBubble() {
  return (
    <Box className={feedRowAgent} style={{ paddingBottom: 0 }}>
      <Box className={liveBubble}>
        <span className={liveBubbleDot} />
        <span className={liveBubbleDot} />
        <span className={liveBubbleDot} />
      </Box>
    </Box>
  );
}

function CaptainThinkingBubble({
  thinkingTokens,
  toolsOk,
  toolsFailed,
}: {
  thinkingTokens: number;
  toolsOk: number;
  toolsFailed: number;
}) {
  return (
    <Box className={feedRowAgent} style={{ paddingBottom: 0 }}>
      <Box className={liveBubble}>
        <img
          src={captainAvatar}
          alt="Captain"
          style={{ width: 16, height: 16, borderRadius: "50%", flexShrink: 0 }}
        />
        <Spinner size="1" />
        {thinkingTokens > 0 ? (
          <Text size="1" color="gray">
            ~{thinkingTokens} tokens
          </Text>
        ) : (
          <>
            <span className={liveBubbleDot} />
            <span className={liveBubbleDot} />
            <span className={liveBubbleDot} />
          </>
        )}
        {toolsOk > 0 && (
          <Text size="1" style={{ color: "var(--green-11)" }}>
            {toolsOk}✓
          </Text>
        )}
        {toolsFailed > 0 && (
          <Text size="1" style={{ color: "var(--red-11)" }}>
            {toolsFailed}✗
          </Text>
        )}
      </Box>
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
    let thinkingChars = 0;
    for (const b of turnBlocks) {
      if (b.role.tag !== "Captain") continue;
      if (b.block.tag === "Text" && b.block.source.tag === "AgentThought") {
        thinkingChars += b.block.text.length;
      } else if (b.block.tag === "ToolCall") {
        if (b.block.status.tag === "Success") toolsOk++;
        else if (b.block.status.tag === "Failure") toolsFailed++;
      }
    }
    thinkingTokens = Math.ceil(thinkingChars / 4);
  }

  let lastUnresolvedPermBlockId: string | undefined;
  for (const entry of visibleBlocks) {
    if (entry.block.tag === "Permission" && !entry.block.resolution) {
      lastUnresolvedPermBlockId = entry.blockId;
    }
  }

  const segments = buildSegments(visibleBlocks);

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
                />
                {debugMode && (
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
                        minHeight: "4rem",
                        maxHeight: "12rem",
                        overflowY: "auto",
                        fontFamily: "monospace",
                        fontSize: "var(--font-size-1)",
                        whiteSpace: "pre",
                        marginTop: "var(--space-1)",
                        background: "var(--gray-a2)",
                        borderRadius: "var(--radius-1)",
                        padding: "var(--space-1)",
                      }}
                    >
                      {JSON.stringify(
                        {
                          blockId: seg.entry.blockId,
                          role: seg.entry.role,
                          block: seg.entry.block,
                        },
                        null,
                        2,
                      )}
                    </Box>
                  </Box>
                )}
              </Fragment>
            );
          })}
        </Box>

        <Box className={liveBubblesRow}>
          {captainWorking && (
            <CaptainThinkingBubble
              thinkingTokens={thinkingTokens}
              toolsOk={toolsOk}
              toolsFailed={toolsFailed}
            />
          )}
          {mateWorking && <LiveBubble />}
        </Box>
      </Box>
    </Box>
  );
}
