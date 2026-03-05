import { useState } from "react";
import { Badge, Box, Button, Callout, Flex, Progress, Spinner, Text } from "@radix-ui/themes";
import { ArrowsClockwise, Warning } from "@phosphor-icons/react";
import type { AgentSnapshot, ContentBlock, PermissionResolution } from "../generated/ship";
import { useSessionEvents } from "../hooks/useSessionEvents";
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
  sessionId: string;
  agent: AgentSnapshot;
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

function latestPlan(events: ContentBlock[]): PlanUpdateBlock | undefined {
  let last: PlanUpdateBlock | undefined;
  for (const e of events) {
    if (e.tag === "PlanUpdate") last = e;
  }
  return last;
}

// r[ui.agent-header.layout]
export function AgentPanel({ sessionId, agent }: Props) {
  const events = useSessionEvents(sessionId, agent.role);
  const plan = latestPlan(events);
  const [resolvedPerms, setResolvedPerms] = useState<Record<number, PermissionResolution>>({});

  const contextPct = agent.context_remaining_percent;
  const contextLow = contextPct !== null && contextPct < 20;

  let lastUnresolvedPermIdx: number | undefined;
  events.forEach((e, i) => {
    if (e.tag === "Permission" && !e.resolution && !resolvedPerms[i]) {
      lastUnresolvedPermIdx = i;
    }
  });

  function renderBlock(block: ContentBlock, idx: number) {
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
        const resolution: PermissionResolution | null = resolvedPerms[idx] ?? block.resolution;
        const resolvedBlock: Extract<ContentBlock, { tag: "Permission" }> = {
          ...block,
          resolution,
        };
        const isActive = idx === lastUnresolvedPermIdx;
        return (
          <PermissionBlock
            block={resolvedBlock}
            onApprove={
              isActive
                ? () => setResolvedPerms((r) => ({ ...r, [idx]: { tag: "Approved" } as const }))
                : undefined
            }
            onDeny={
              isActive
                ? () => setResolvedPerms((r) => ({ ...r, [idx]: { tag: "Denied" } as const }))
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

      <Box className={eventStream}>
        {events
          .map((block, i) => [block, i] as const)
          .filter(([block]) => block.tag !== "PlanUpdate")
          .map(([block, i]) => (
            <Box key={i}>{renderBlock(block, i)}</Box>
          ))}
      </Box>
    </Box>
  );
}
