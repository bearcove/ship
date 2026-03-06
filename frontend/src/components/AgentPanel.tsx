import { useRef, useEffect } from "react";
import { Box, Flex, Spinner, Text } from "@radix-ui/themes";
import type { AgentSnapshot, ContentBlock } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { TextBlock } from "./blocks/TextBlock";
import { ToolCallBlock } from "./blocks/ToolCallBlock";
import { PlanUpdateBlock as PlanUpdateBlockComponent } from "./blocks/PlanUpdateBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import { getShipClient } from "../api/client";
import {
  agentPanelRoot,
  agentPanelScrollArea,
  eventStream,
  stickyPlan,
} from "../styles/session-view.css";

interface Props {
  sessionId: string;
  agent: AgentSnapshot;
  blocks: BlockEntry[];
  loading?: boolean;
  loadingLabel?: string;
}

type PlanUpdateBlock = Extract<ContentBlock, { tag: "PlanUpdate" }>;

function latestPlan(entries: BlockEntry[]): PlanUpdateBlock | undefined {
  let last: PlanUpdateBlock | undefined;
  for (const entry of entries) {
    if (entry.block.tag === "PlanUpdate") last = entry.block as PlanUpdateBlock;
  }
  return last;
}

// r[ui.event-stream.grouping]
// r[view.agent-panel.state]
// r[ui.block.plan.position]
// r[ui.block.plan.filtering]
export function AgentPanel({ sessionId, agent, blocks, loading, loadingLabel }: Props) {
  const plan = latestPlan(blocks);

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
      case "Text":
        return <TextBlock block={block} />;
      case "ToolCall":
        return <ToolCallBlock block={block} />;
      case "PlanUpdate":
        return null;
      case "Error":
        return <ErrorBlock block={block} agentState={agent.state} />;
      // r[ui.permission.actions]
      case "Permission": {
        const isActive = blockId === lastUnresolvedPermBlockId;
        const permissionId =
          isActive && agent.state.tag === "AwaitingPermission"
            ? agent.state.request.permission_id
            : null;

        async function resolve(approved: boolean) {
          if (!permissionId) return;
          const client = await getShipClient();
          await client.resolvePermission(sessionId, permissionId, approved);
        }

        return (
          <PermissionBlock
            block={block}
            onApprove={permissionId ? () => resolve(true) : undefined}
            onDeny={permissionId ? () => resolve(false) : undefined}
          />
        );
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
          {blocks
            .filter((entry) => entry.block.tag !== "PlanUpdate")
            .map((entry) => (
              <Box key={entry.blockId}>{renderBlock(entry)}</Box>
            ))}
        </Box>
      </Box>
    </Box>
  );
}
