import { useState, useRef, useEffect } from "react";
import { Badge, Box, Button, Callout, Flex, Progress, Spinner, Text } from "@radix-ui/themes";
import { ArrowsClockwise, Warning } from "@phosphor-icons/react";
import type { AgentSnapshot, ContentBlock, PermissionResolution } from "../generated/ship";
import type { BlockEntry } from "../state/blockStore";
import { TextBlock } from "./blocks/TextBlock";
import { ToolCallBlock } from "./blocks/ToolCallBlock";
import { PlanUpdateBlock as PlanUpdateBlockComponent } from "./blocks/PlanUpdateBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { PermissionBlock } from "./blocks/PermissionBlock";
import {
  agentHeader,
  agentHeaderRow,
  agentPanelRoot,
  eventStream,
  stickyPlan,
} from "../styles/session-view.css";

interface Props {
  agent: AgentSnapshot;
  blocks: BlockEntry[];
}

type PlanUpdateBlock = Extract<ContentBlock, { tag: "PlanUpdate" }>;

function AgentStateBadge({ agent }: { agent: AgentSnapshot }) {
  switch (agent.state.tag) {
    case "Working":
      return (
        <Badge color="blue" size="1">
          <Spinner size="1" />
          Working
        </Badge>
      );
    case "Idle":
      return (
        <Badge color="gray" size="1">
          Idle
        </Badge>
      );
    case "AwaitingPermission":
      return (
        <Badge color="amber" size="1">
          Awaiting Permission
        </Badge>
      );
    case "ContextExhausted":
      return (
        <Badge color="red" size="1">
          Context Exhausted
        </Badge>
      );
    case "Error":
      return (
        <Badge color="red" size="1">
          <Warning size={10} />
          Error
        </Badge>
      );
  }
}

function latestPlan(entries: BlockEntry[]): PlanUpdateBlock | undefined {
  let last: PlanUpdateBlock | undefined;
  for (const entry of entries) {
    if (entry.block.tag === "PlanUpdate") last = entry.block as PlanUpdateBlock;
  }
  return last;
}

// r[ui.agent-header.layout]
// r[ui.event-stream.grouping]
export function AgentPanel({ agent, blocks }: Props) {
  const plan = latestPlan(blocks);
  const [resolvedPerms, setResolvedPerms] = useState<Record<string, PermissionResolution>>({});

  const contextPct = agent.context_remaining_percent;
  const contextLow = contextPct !== null && contextPct < 20;

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
      <Box className={agentHeader}>
        <Flex className={agentHeaderRow}>
          <Badge color={agent.kind.tag === "Claude" ? "violet" : "cyan"} variant="soft">
            {agent.kind.tag}
          </Badge>
          <Text size="1" color="gray">
            {agent.role.tag}
          </Text>
          <Box ml="auto">
            <AgentStateBadge agent={agent} />
          </Box>
        </Flex>

        {contextPct !== null && agent.state.tag !== "ContextExhausted" && (
          <Progress value={contextPct} color={contextLow ? "red" : "blue"} size="1" />
        )}

        {/* r[context.warning] */}
        {contextLow && agent.state.tag !== "ContextExhausted" && (
          <Callout.Root color="red" size="1" variant="soft">
            <Callout.Icon>
              <Warning size={14} />
            </Callout.Icon>
            <Callout.Text>
              Context window below 20% — agent may need to be rotated soon.
            </Callout.Text>
          </Callout.Root>
        )}

        {/* r[context.manual-rotation] */}
        {agent.state.tag === "ContextExhausted" && (
          <Callout.Root color="red" size="1">
            <Callout.Icon>
              <Warning size={14} />
            </Callout.Icon>
            <Callout.Text>Context window exhausted — agent cannot continue.</Callout.Text>
            <Button size="1" color="red" variant="soft" mt="2">
              <ArrowsClockwise size={12} />
              Rotate Agent
            </Button>
          </Callout.Root>
        )}

        {/* r[ui.error.agent] */}
        {agent.state.tag === "Error" && (
          <Callout.Root color="red" size="1">
            <Callout.Icon>
              <Warning size={14} />
            </Callout.Icon>
            <Callout.Text>{agent.state.message}</Callout.Text>
            <Button size="1" color="red" variant="soft" mt="2">
              <ArrowsClockwise size={12} />
              Retry Agent
            </Button>
          </Callout.Root>
        )}
      </Box>

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
