import { Badge, Box, Callout, Flex, Progress, Spinner, Text } from "@radix-ui/themes";
import { Warning } from "@phosphor-icons/react";
import type { AgentInfo, ContentBlock, PlanUpdateBlock } from "../types";
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
  agent: AgentInfo;
}

function AgentStateBadge({ agent }: { agent: AgentInfo }) {
  switch (agent.state) {
    case "working":
      return (
        <Badge color="blue" size="1">
          <Spinner size="1" />
          Working
        </Badge>
      );
    case "idle":
      return (
        <Badge color="gray" size="1">
          Idle
        </Badge>
      );
    case "awaiting-permission":
      return (
        <Badge color="amber" size="1">
          Awaiting Permission
        </Badge>
      );
    case "context-exhausted":
      return (
        <Badge color="red" size="1">
          Context Exhausted
        </Badge>
      );
    case "error":
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
    if (e.type === "plan-update") last = e;
  }
  return last;
}

function renderBlock(block: ContentBlock, agentState: AgentInfo["state"]) {
  switch (block.type) {
    case "text":
      return <TextBlock block={block} />;
    case "tool-call":
      return <ToolCallBlock block={block} />;
    case "plan-update":
      return null;
    case "error":
      return <ErrorBlock block={block} agentState={agentState} />;
    case "permission":
      return <PermissionBlock block={block} />;
  }
}

export function AgentPanel({ sessionId, agent }: Props) {
  const events = useSessionEvents(sessionId, agent.role);
  const plan = latestPlan(events);

  const contextPct = agent.context
    ? Math.round(((agent.context.total - agent.context.used) / agent.context.total) * 100)
    : null;

  const contextLow = contextPct !== null && contextPct < 20;

  return (
    <Box className={agentPanelRoot}>
      <Box className={agentHeader}>
        <Flex className={agentHeaderRow}>
          <Badge color={agent.kind === "claude" ? "violet" : "cyan"} variant="soft">
            {agent.kind === "claude" ? "Claude" : "Codex"}
          </Badge>
          <Text size="1" color="gray" style={{ textTransform: "capitalize" }}>
            {agent.role}
          </Text>
          <Box ml="auto">
            <AgentStateBadge agent={agent} />
          </Box>
        </Flex>
        {contextPct !== null && (
          <Progress value={contextPct} color={contextLow ? "red" : "blue"} size="1" />
        )}
        {contextLow && (
          <Callout.Root color="red" size="1" variant="soft">
            <Callout.Icon>
              <Warning size={14} />
            </Callout.Icon>
            <Callout.Text>Context window below 20% — agent may need to be reset soon.</Callout.Text>
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
          .filter((e) => e.type !== "plan-update")
          .map((block) => (
            <Box key={block.id}>{renderBlock(block, agent.state)}</Box>
          ))}
      </Box>
    </Box>
  );
}
