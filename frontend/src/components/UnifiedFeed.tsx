import { Fragment, useState, useRef, useEffect } from "react";
import { Box, Flex, Spinner, Text } from "@radix-ui/themes";
import { ArrowDown, CaretRight, CaretDown } from "@phosphor-icons/react";
import type {
  AgentSnapshot,
  ContentBlock,
  Role,
  SessionStartupState,
  TaskStatus,
} from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { TextBlock } from "./blocks/TextBlock";
import { ToolCallBlock } from "./blocks/ToolCallBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import { getShipClient } from "../api/client";
import captainAvatar from "../assets/avatars/captain.png";
import mateAvatar from "../assets/avatars/mate.png";
import {
  agentAvatar,
  agentAvatarSpacer,
  feedBubble,
  feedBubbleCol,
  feedBubbleColUser,
  feedBubbleMate,
  feedBubbleUser,
  feedTimestamp,
  feedRowAgent,
  feedRowUser,
  feedSystemMessage,
  feedSystemMessageText,
  feedToolGroup,
  feedToolGroupBody,
  feedToolGroupHeader,
  feedToolGroupHeaderExpanded,
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

// ─── Feed segmentation ────────────────────────────────────────────────────────

type SingleSegment = { kind: "single"; entry: BlockEntry };
type ToolGroupSegment = { kind: "tool-group"; role: Role; entries: BlockEntry[] };
type FeedSegment = SingleSegment | ToolGroupSegment;

function buildSegments(blocks: BlockEntry[]): FeedSegment[] {
  const visible = blocks.filter((b) => b.block.tag !== "PlanUpdate");
  const segments: FeedSegment[] = [];
  let i = 0;
  while (i < visible.length) {
    const entry = visible[i];
    if (entry.block.tag === "ToolCall") {
      const group: BlockEntry[] = [entry];
      let j = i + 1;
      while (
        j < visible.length &&
        visible[j].block.tag === "ToolCall" &&
        visible[j].role.tag === entry.role.tag
      ) {
        group.push(visible[j]);
        j++;
      }
      segments.push({ kind: "tool-group", role: entry.role, entries: group });
      i = j;
    } else {
      segments.push({ kind: "single", entry });
      i++;
    }
  }
  return segments;
}

function segmentLastTimestamp(seg: FeedSegment): string | null | undefined {
  if (seg.kind === "tool-group") return seg.entries.at(-1)?.timestamp;
  return seg.entry.timestamp;
}

// Returns the "agent side" role of a segment, or null if it's a user message.
function segmentAgentRole(seg: FeedSegment): Role | null {
  if (seg.kind === "tool-group") return seg.role;
  const { block, role } = seg.entry;
  if (block.tag === "Text" && block.source.tag === "Human" && !isSystemInjection(block)) {
    return null; // real user message → right side, no avatar
  }
  return role;
}

// ─── Avatar ───────────────────────────────────────────────────────────────────

function Avatar({ role, show }: { role: Role; show: boolean }) {
  if (!show) return <div className={agentAvatarSpacer} />;
  const src = role.tag === "Captain" ? captainAvatar : mateAvatar;
  const label = role.tag === "Captain" ? "Captain" : "Mate";
  return <img src={src} className={agentAvatar} alt={label} />;
}

function UserAvatar({ url }: { url: string | null }) {
  if (!url) return <div className={userAvatarSpacer} />;
  return <img src={url} className={userAvatar} alt="You" />;
}

// ─── Tool group ───────────────────────────────────────────────────────────────

function ToolGroup({
  entries,
  role,
  showAvatar,
}: {
  entries: BlockEntry[];
  role: Role;
  showAvatar: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const count = entries.length;

  return (
    <Box className={feedRowAgent}>
      <Avatar role={role} show={showAvatar} />
      <Box className={feedToolGroup}>
        <div
          className={`${feedToolGroupHeader}${expanded ? ` ${feedToolGroupHeaderExpanded}` : ""}`}
          onClick={() => setExpanded((v) => !v)}
        >
          {expanded ? <CaretDown size={11} /> : <CaretRight size={11} />}
          <Text size="1" color="gray">
            {count} tool call{count !== 1 ? "s" : ""}
          </Text>
        </div>
        {expanded && (
          <Box className={feedToolGroupBody}>
            {entries.map((e) => (
              <ToolCallBlock
                key={e.blockId}
                block={e.block as Extract<ContentBlock, { tag: "ToolCall" }>}
              />
            ))}
          </Box>
        )}
      </Box>
    </Box>
  );
}

// ─── Thought block ────────────────────────────────────────────────────────────

function ThoughtBlock({
  block,
  role,
  showAvatar,
  prevTimestamp,
  timestamp,
}: {
  block: TextBlockType;
  role: Role;
  showAvatar: boolean;
  prevTimestamp?: string | null;
  timestamp?: string | null;
}) {
  const [expanded, setExpanded] = useState(false);

  let durationLabel = "Thought";
  if (prevTimestamp && timestamp) {
    const secs = Math.round(
      (new Date(timestamp).getTime() - new Date(prevTimestamp).getTime()) / 1000,
    );
    if (secs > 0) durationLabel = `Thought for ${secs}s`;
  }

  return (
    <Box className={feedRowAgent}>
      <Avatar role={role} show={showAvatar} />
      <Box className={feedToolGroup}>
        <div
          className={`${feedToolGroupHeader}${expanded ? ` ${feedToolGroupHeaderExpanded}` : ""}`}
          onClick={() => setExpanded((v) => !v)}
          role="button"
          aria-expanded={expanded}
        >
          {expanded ? <CaretDown size={11} /> : <CaretRight size={11} />}
          <Text size="1" color="gray">
            {durationLabel}
          </Text>
        </div>
        {expanded && (
          <Box className={feedToolGroupBody}>
            <TextBlock block={block} />
          </Box>
        )}
      </Box>
    </Box>
  );
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
      if (isHuman) {
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

      // Thought block — collapsible, no bubble
      if (isThought) {
        return (
          <ThoughtBlock
            block={block as TextBlockType}
            role={role}
            showAvatar={showAvatar}
            prevTimestamp={prevTimestamp}
            timestamp={entry.timestamp}
          />
        );
      }

      // Agent message — left side with avatar
      return (
        <Box className={feedRowAgent}>
          <Avatar role={role} show={showAvatar} />
          <Box className={feedBubbleCol}>
            <Box className={`${feedBubble}${isCaptain ? "" : ` ${feedBubbleMate}`}`}>
              <TextBlock block={block as TextBlockType} />
            </Box>
            {isLast && entry.timestamp && (
              <Text className={feedTimestamp}>{formatTime(entry.timestamp)}</Text>
            )}
          </Box>
        </Box>
      );
    }

    case "ToolCall":
      return (
        <Box className={feedRowAgent}>
          <Avatar role={role} show={showAvatar} />
          <Box className={feedToolGroup}>
            <ToolCallBlock block={block} />
          </Box>
        </Box>
      );

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

function LiveBubble({ role }: { role: Role }) {
  return (
    <Box className={feedRowAgent} style={{ paddingBottom: 0 }}>
      <Avatar role={role} show />
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

  let lastUnresolvedPermBlockId: string | undefined;
  for (const entry of blocks) {
    if (entry.block.tag === "Permission" && !entry.block.resolution) {
      lastUnresolvedPermBlockId = entry.blockId;
    }
  }

  const segments = buildSegments(blocks);

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

          {segments.map((seg, idx) => {
            const showAvatar = showAvatarAt.has(idx);

            if (seg.kind === "tool-group") {
              return (
                <ToolGroup
                  key={idx}
                  entries={seg.entries}
                  role={seg.role}
                  showAvatar={showAvatar}
                />
              );
            }

            const agentForBlock = seg.entry.role.tag === "Captain" ? captain : mate;
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

        <Box className={liveBubblesRow}>
          {captainWorking && <LiveBubble role={{ tag: "Captain" }} />}
          {mateWorking && <LiveBubble role={{ tag: "Mate" }} />}
        </Box>
      </Box>
    </Box>
  );
}
