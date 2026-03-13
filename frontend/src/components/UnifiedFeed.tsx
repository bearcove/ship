import { Fragment, useState, useRef, useEffect } from "react";
import { Box, Flex, Spinner, Text } from "@radix-ui/themes";
import { ArrowDown } from "@phosphor-icons/react";
import type {
  AgentKind,
  AgentSnapshot,
  ContentBlock,
  Role,
  SessionStartupState,
  TaskStatus,
} from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { AgentKindIcon } from "./AgentKindIcon";
import { TextBlock } from "./blocks/TextBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import { ImageBlock } from "./blocks/ImageBlock";
import { getShipClient } from "../api/client";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import {
  agentAvatar,
  agentAvatarBadge,
  agentAvatarSpacer,
  agentAvatarWrapper,
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
  feedSystemMessageText,
  liveBubble,
  liveBubbleDot,
  liveBubblesRow,
  startupFeedBody,
  startupFeedItem,
  feedMessageMeta,
  scrollToBottomBtn,
  unifiedFeedRoot,
  unifiedFeedScroll,
  unifiedFeedStream,
  userAvatar,
  userAvatarSpacer,
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

function systemInjectionLabel(role: Role): string {
  return role.tag === "Captain" ? "Captain was prompted" : "Mate was assigned a task";
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

function segmentLastTimestamp(seg: FeedSegment): string | null | undefined {
  return seg.entry.timestamp;
}

// Returns the "agent side" role of a segment, or null if it's a real user message.
// Captain-to-mate relay messages (role=Mate, source=Human) are attributed to the
// Captain so they group with other captain output and show the captain avatar.
function segmentAgentRole(seg: FeedSegment): Role | null {
  const { block, role } = seg.entry;
  if (block.tag === "Text" && block.source.tag === "Human" && !isSystemInjection(block)) {
    if (role.tag === "Captain") return null; // real user message → right side, no avatar
    return { tag: "Captain" }; // captain relaying to mate → left side, captain avatar
  }
  if (block.tag === "Image" && role.tag === "Captain") {
    return null; // user-sent image → right side, no avatar
  }
  return role;
}

// ─── Avatar ───────────────────────────────────────────────────────────────────

function Avatar({ role, show, kind }: { role: Role; show: boolean; kind?: AgentKind }) {
  if (!show) return <div className={agentAvatarSpacer} />;
  const src = role.tag === "Captain" ? captainAvatar : mateAvatar;
  const label = role.tag === "Captain" ? "Captain" : "Mate";
  if (kind) {
    return (
      <div className={agentAvatarWrapper}>
        <img src={src} className={agentAvatar} alt={label} />
        <div className={agentAvatarBadge}>
          <AgentKindIcon kind={kind} />
        </div>
      </div>
    );
  }
  return <img src={src} className={agentAvatar} alt={label} />;
}

function UserAvatar({ url }: { url: string | null }) {
  if (!url) return <div className={userAvatarSpacer} />;
  return <img src={url} className={userAvatar} alt="You" />;
}

// ─── Single block ─────────────────────────────────────────────────────────────

function SingleBlock({
  entry,
  sessionId,
  lastUnresolvedPermBlockId,
  agentForBlock,
  showAvatar,
  userAvatarUrl,
  isLast,
  prevTimestamp,
}: {
  entry: BlockEntry;
  sessionId: string;
  lastUnresolvedPermBlockId: string | undefined;
  agentForBlock: AgentSnapshot | null;
  showAvatar: boolean;
  userAvatarUrl: string | null;
  isLast: boolean;
  prevTimestamp?: string | null;
}) {
  const { block, blockId, role } = entry;
  const isCaptain = role.tag === "Captain";
  const kind = agentForBlock?.kind;

  switch (block.tag) {
    case "Text": {
      if (block.text.trim() === "") return null;

      const isHuman = block.source.tag === "Human";
      const isThought = block.source.tag === "AgentThought";

      // Server-injected system message — collapse to a label
      if (isHuman && isSystemInjection(block)) {
        return (
          <Box className={feedSystemMessage}>
            <Text className={feedSystemMessageText}>{systemInjectionLabel(role)}</Text>
          </Box>
        );
      }

      // Real user message — right side with avatar
      if (isHuman && role.tag === "Captain") {
        return (
          <Box className={feedRowUser}>
            <Box className={`${feedBubbleCol} ${feedBubbleColUser}`}>
              <Box className={`${feedBubble} ${feedBubbleUser}`}>
                <TextBlock block={block as TextBlockType} />
              </Box>
              {isLast && entry.timestamp && (
                <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
              )}
            </Box>
            <UserAvatar url={userAvatarUrl} />
          </Box>
        );
      }

      // Captain relaying to mate — left side, captain avatar, amber tint
      if (isHuman && role.tag === "Mate") {
        return (
          <Box className={feedRowAgent}>
            <Avatar role={{ tag: "Captain" }} show={showAvatar} kind={kind} />
            <Box className={feedBubbleCol}>
              <Box className={`${feedBubble} ${feedBubbleRelay}`}>
                <TextBlock block={block as TextBlockType} speakable />
              </Box>
              {isLast && entry.timestamp && (
                <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
              )}
            </Box>
          </Box>
        );
      }

      // Thought block — hidden
      if (isThought) {
        return null;
      }

      // Agent message — left side with avatar
      return (
        <Box className={feedRowAgent}>
          <Avatar role={role} show={showAvatar} kind={kind} />
          <Box className={feedBubbleCol}>
            <Box
              className={`${feedBubble}${isCaptain ? ` ${feedBubbleCaptain}` : ` ${feedBubbleMate}`}`}
            >
              <TextBlock block={block as TextBlockType} speakable />
            </Box>
            {isLast && entry.timestamp && (
              <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
            )}
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
      // Mate/relay image — left side, captain avatar
      return (
        <Box className={feedRowAgent}>
          <Avatar role={{ tag: "Captain" }} show={showAvatar} kind={kind} />
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

function LiveBubble({ role, kind }: { role: Role; kind?: AgentKind }) {
  return (
    <Box className={feedRowAgent} style={{ paddingBottom: 0 }}>
      <Avatar role={role} show kind={kind} />
      <Box className={liveBubble}>
        <span className={liveBubbleDot} />
        <span className={liveBubbleDot} />
        <span className={liveBubbleDot} />
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
  taskStatus: TaskStatus | null;
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
  taskStatus,
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

  function kindForRole(role: Role): AgentKind | undefined {
    return role.tag === "Captain" ? captain?.kind : mate?.kind;
  }

  const MAX_RENDERED_BLOCKS = 80;
  const truncated = blocks.length > MAX_RENDERED_BLOCKS;
  const visibleBlocks = truncated ? blocks.slice(blocks.length - MAX_RENDERED_BLOCKS) : blocks;

  let lastUnresolvedPermBlockId: string | undefined;
  for (const entry of visibleBlocks) {
    if (entry.block.tag === "Permission" && !entry.block.resolution) {
      lastUnresolvedPermBlockId = entry.blockId;
    }
  }

  const segments = buildSegments(visibleBlocks);

  // Determine which segments show the avatar: only the first in a consecutive
  // run from the same agent role.
  const showAvatarAt = new Set<number>();
  for (let i = 0; i < segments.length; i++) {
    const thisRole = segmentAgentRole(segments[i]);
    if (!thisRole) continue; // user message, no avatar
    const prevRole = i > 0 ? segmentAgentRole(segments[i - 1]) : null;
    if (!prevRole || prevRole.tag !== thisRole.tag) {
      showAvatarAt.add(i);
    }
  }

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
            const showAvatar = showAvatarAt.has(idx);
            const agentRole = segmentAgentRole(seg);
            const agentForBlock =
              agentRole?.tag === "Captain"
                ? captain
                : agentRole?.tag === "Mate"
                  ? mate
                  : seg.entry.role.tag === "Captain"
                    ? captain
                    : mate;
            const prevTimestamp = idx > 0 ? segmentLastTimestamp(segments[idx - 1]) : null;
            return (
              <Fragment key={seg.entry.blockId}>
                <SingleBlock
                  entry={seg.entry}
                  sessionId={sessionId}
                  lastUnresolvedPermBlockId={lastUnresolvedPermBlockId}
                  agentForBlock={agentForBlock}
                  showAvatar={showAvatar}
                  userAvatarUrl={userAvatarUrl}
                  isLast={idx === segments.length - 1}
                  prevTimestamp={prevTimestamp}
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

        {taskCompletedDuration != null &&
          taskStatus &&
          (taskStatus.tag === "Accepted" || taskStatus.tag === "Cancelled") && (
            <Box className={feedSystemMessage}>
              <Text className={feedSystemMessageText}>
                Task {taskStatus.tag === "Accepted" ? "completed" : "cancelled"} in{" "}
                {formatDuration(taskCompletedDuration)}
              </Text>
            </Box>
          )}

        <Box className={liveBubblesRow}>
          {captainWorking && (
            <LiveBubble role={{ tag: "Captain" }} kind={kindForRole({ tag: "Captain" })} />
          )}
          {mateWorking && <LiveBubble role={{ tag: "Mate" }} kind={kindForRole({ tag: "Mate" })} />}
        </Box>
      </Box>
    </Box>
  );
}
