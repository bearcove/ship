import { Box, Button, Callout, Flex, Progress, Text } from "@radix-ui/themes";
import { ArrowsClockwise, Warning } from "@phosphor-icons/react";
import type { AgentSnapshot } from "../generated/ship";
import { AgentKindIcon } from "./AgentKindIcon";
import { agentHeader, agentHeaderRow } from "../styles/session-view.css";

interface Props {
  agent: AgentSnapshot;
}

// r[ui.agent-header.layout]
// r[view.agent-panel.state]
export function AgentHeader({ agent }: Props) {
  const contextPct = agent.context_remaining_percent;
  const contextLow = contextPct !== null && contextPct < 20;

  return (
    <Box className={agentHeader}>
      <Flex className={agentHeaderRow}>
        <AgentKindIcon kind={agent.kind} />
        <Text size="2" weight="medium">
          {agent.role.tag}
        </Text>
        {/* r[ui.agent-header.context-bar] */}
        {contextPct !== null && agent.state.tag !== "ContextExhausted" && (
          <Box style={{ width: 56, flexShrink: 0, marginLeft: "auto" }}>
            <Progress value={contextPct} color={contextLow ? "red" : undefined} size="1" />
          </Box>
        )}
      </Flex>

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
