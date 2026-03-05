import { Badge, Box, Button, Callout, Flex, Progress, Spinner, Text } from "@radix-ui/themes";
import { ArrowsClockwise, Warning } from "@phosphor-icons/react";
import type { AgentSnapshot } from "../generated/ship";
import { agentHeader, agentHeaderRow } from "../styles/session-view.css";

interface Props {
  agent: AgentSnapshot;
}

// r[ui.agent-header.state-indicator]
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

// r[ui.agent-header.layout]
// r[view.agent-panel.state]
export function AgentHeader({ agent }: Props) {
  const contextPct = agent.context_remaining_percent;
  const contextLow = contextPct !== null && contextPct < 20;

  return (
    <Box className={agentHeader}>
      <Flex className={agentHeaderRow}>
        <Text size="2" weight="medium">
          {agent.role.tag}
        </Text>
        <Badge color={agent.kind.tag === "Claude" ? "violet" : "cyan"} variant="soft" size="1">
          {agent.kind.tag}
        </Badge>
        <Box ml="auto">
          <AgentStateBadge agent={agent} />
        </Box>
      </Flex>

      {/* r[ui.agent-header.context-bar] */}
      {contextPct !== null && agent.state.tag !== "ContextExhausted" && (
        <Progress value={contextPct} color={contextLow ? "red" : "blue"} size="1" />
      )}

      {/* r[context.warning] */}
      {contextLow && agent.state.tag !== "ContextExhausted" && (
        <Callout.Root color="red" size="1" variant="soft">
          <Callout.Icon>
            <Warning size={14} />
          </Callout.Icon>
          <Callout.Text>Context window below 20% — agent may need to be rotated soon.</Callout.Text>
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
  );
}
