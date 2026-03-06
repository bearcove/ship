import { Fragment, useRef, useEffect } from "react";
import { Box, Flex, Spinner, Text } from "@radix-ui/themes";
import type {
  AgentSnapshot,
  ContentBlock,
  SessionStartupState,
  TaskStatus,
} from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { TextBlock } from "./blocks/TextBlock";
import { ToolCallBlock } from "./blocks/ToolCallBlock";
import { PlanUpdateBlock as PlanUpdateBlockComponent } from "./blocks/PlanUpdateBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import { InlineAgentComposer } from "./InlineAgentComposer";
import { getShipClient } from "../api/client";
import {
  agentPanelRoot,
  agentPanelScrollArea,
  eventStream,
  feedMessageCard,
  feedMessageCardAgent,
  feedMessageCardThought,
  feedMessageCardUser,
  feedMessageMeta,
  feedMessageRow,
  stickyPlan,
  startupFeedBody,
  startupFeedItem,
} from "../styles/session-view.css";

interface Props {
  sessionId: string;
  agent: AgentSnapshot;
  blocks: BlockEntry[];
  debugMode?: boolean;
  loading?: boolean;
  loadingLabel?: string;
  startupState: SessionStartupState | null;
  taskStatus: TaskStatus | null;
}

type PlanUpdateBlock = Extract<ContentBlock, { tag: "PlanUpdate" }>;
type TextBlockType = Extract<ContentBlock, { tag: "Text" }>;

function latestPlan(entries: BlockEntry[]): PlanUpdateBlock | undefined {
  let last: PlanUpdateBlock | undefined;
  for (const entry of entries) {
    if (entry.block.tag === "PlanUpdate") last = entry.block as PlanUpdateBlock;
  }
  return last;
}

function StartupFeedState({ startupState }: { startupState: SessionStartupState }) {
  const tone = startupState.tag === "Failed" ? "error" : "neutral";
  const title = startupState.tag === "Failed" ? "Session startup failed" : "Session startup";
  let detail = "Waiting to begin startup.";
  if (startupState.tag === "Running" || startupState.tag === "Failed") {
    detail = startupState.message;
  }

  return (
    <Box className={startupFeedItem} data-tone={tone}>
      {startupState.tag === "Failed" ? null : <Spinner size="1" />}
      <Box className={startupFeedBody}>
        <Text className={feedMessageMeta}>{title}</Text>
        <Text size="2">{detail}</Text>
      </Box>
    </Box>
  );
}

// r[ui.event-stream.grouping]
// r[view.agent-panel.state]
// r[ui.block.plan.position]
// r[ui.block.plan.filtering]
export function AgentPanel({
  sessionId,
  agent,
  blocks,
  debugMode = false,
  loading,
  loadingLabel,
  startupState,
  taskStatus,
}: Props) {
  const plan = latestPlan(blocks);
  const showStartupFeed = agent.role.tag === "Captain" && startupState?.tag !== "Ready";

  let lastUnresolvedPermBlockId: string | undefined;
  for (const entry of blocks) {
    if (entry.block.tag === "Permission" && !entry.block.resolution) {
      lastUnresolvedPermBlockId = entry.blockId;
    }
  }

  // r[ui.event-stream.layout]
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickyScroll = useRef(true);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el || !stickyScroll.current) return;
    el.scrollTop = el.scrollHeight;
  }, [blocks]);

  function handleScroll() {
    const el = scrollRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 32;
    stickyScroll.current = atBottom;
  }

  function renderBlock(entry: BlockEntry) {
    const { block, blockId } = entry;
    switch (block.tag) {
      case "Text": {
        const cardClassName =
          block.source.tag === "Human"
            ? feedMessageCardUser
            : block.source.tag === "AgentThought"
              ? feedMessageCardThought
              : feedMessageCardAgent;
        return (
          <Box className={feedMessageRow}>
            <Box className={`${feedMessageCard} ${cardClassName}`}>
              <TextBlock block={block as TextBlockType} />
            </Box>
          </Box>
        );
      }
      case "ToolCall":
        return <ToolCallBlock block={block} />;
      case "PlanUpdate":
        return null;
      case "Error":
        return <ErrorBlock block={block} agentState={agent.state} />;
      // r[ui.permission.actions]
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
    }
  }

  return (
    <Box className={agentPanelRoot}>
      {loading && (
        <Flex align="center" gap="2" px="3" py="2" style={{ flexShrink: 0 }}>
          <Spinner size="1" />
          <Text size="1" color="gray">
            {loadingLabel ?? "Replaying events…"}
          </Text>
        </Flex>
      )}

      <Box ref={scrollRef} className={agentPanelScrollArea} onScroll={handleScroll}>
        {plan && (
          <Box className={stickyPlan}>
            <PlanUpdateBlockComponent block={plan} />
          </Box>
        )}

        <Box className={eventStream}>
          {showStartupFeed && startupState && <StartupFeedState startupState={startupState} />}
          {blocks
            .filter((entry) => entry.block.tag !== "PlanUpdate")
            .map((entry) => (
              <Fragment key={entry.blockId}>
                <Box>{renderBlock(entry)}</Box>
                {debugMode && (
                  <Box
                    px="2"
                    pt="1"
                    pb="2"
                    style={{
                      border: "1px dashed var(--gray-a5)",
                      borderRadius: "var(--radius-2)",
                    }}
                  >
                    <Text size="1" color="gray">
                      raw block payload
                    </Text>
                    <Box
                      style={{
                        minHeight: "6rem",
                        maxHeight: "20rem",
                        overflowY: "auto",
                        overflowX: "auto",
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
                          blockId: entry.blockId,
                          role: entry.role,
                          block: entry.block,
                        },
                        null,
                        2,
                      )}
                    </Box>
                  </Box>
                )}
              </Fragment>
            ))}
        </Box>
      </Box>

      <InlineAgentComposer
        sessionId={sessionId}
        role={agent.role}
        agentStateTag={agent.state.tag}
        startupState={startupState}
        taskStatus={taskStatus}
      />
    </Box>
  );
}
