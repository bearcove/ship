import { useState, useRef, useEffect } from "react";
import { Box } from "@radix-ui/themes";
import type { AgentSnapshot, ContentBlock, PermissionResolution } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { TextBlock } from "./blocks/TextBlock";
import { ToolCallBlock } from "./blocks/ToolCallBlock";
import { PlanUpdateBlock as PlanUpdateBlockComponent } from "./blocks/PlanUpdateBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import { agentPanelRoot, eventStream, stickyPlan } from "../styles/session-view.css";

interface Props {
  agent: AgentSnapshot;
  blocks: BlockEntry[];
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
export function AgentPanel({ agent, blocks }: Props) {
  const plan = latestPlan(blocks);
  const [resolvedPerms, setResolvedPerms] = useState<Record<string, PermissionResolution>>({});

  let lastUnresolvedPermBlockId: string | undefined;
  for (const entry of blocks) {
    if (
      entry.block.tag === "Permission" &&
      !entry.block.resolution &&
      !resolvedPerms[entry.blockId]
    ) {
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
      case "Permission": {
        const resolution: PermissionResolution | null = resolvedPerms[blockId] ?? block.resolution;
        const resolvedBlock: Extract<ContentBlock, { tag: "Permission" }> = {
          ...block,
          resolution,
        };
        const isActive = blockId === lastUnresolvedPermBlockId;
        return (
          <PermissionBlock
            block={resolvedBlock}
            onApprove={
              isActive
                ? () => setResolvedPerms((r) => ({ ...r, [blockId]: { tag: "Approved" } as const }))
                : undefined
            }
            onDeny={
              isActive
                ? () => setResolvedPerms((r) => ({ ...r, [blockId]: { tag: "Denied" } as const }))
                : undefined
            }
          />
        );
      }
    }
  }

  return (
    <Box className={agentPanelRoot}>
      {plan && (
        <Box className={stickyPlan}>
          <PlanUpdateBlockComponent block={plan} />
        </Box>
      )}

      <Box ref={scrollRef} className={eventStream} onScroll={handleScroll}>
        {blocks
          .filter((entry) => entry.block.tag !== "PlanUpdate")
          .map((entry) => (
            <Box key={entry.blockId}>{renderBlock(entry)}</Box>
          ))}
      </Box>
    </Box>
  );
}
