import { Box, Button, Callout, Flex, Text } from "@radix-ui/themes";
import { ArrowsClockwise, Warning } from "@phosphor-icons/react";
import type { AgentSnapshot } from "../generated/ship";
import { AgentKindIcon } from "./AgentKindIcon";
import { AgentModelPicker } from "./AgentModelPicker";
import { AgentEffortPicker } from "./AgentEffortPicker";
import { getShipClient } from "../api/client";
import {
  agentHeader,
  agentHeaderAvatar,
  agentHeaderAvatarFallback,
  agentHeaderBody,
  agentHeaderContext,
  agentHeaderContextArc,
  agentHeaderContextSvg,
  agentHeaderContextTrack,
  agentHeaderMain,
  agentHeaderRole,
  agentHeaderSummaryRow,
} from "../styles/session-view.css";

interface Props {
  sessionId: string;
  agent: AgentSnapshot;
  avatarSrc?: string;
}

// r[ui.agent-header.layout]
// r[view.agent-panel.state]
export function AgentHeader({ sessionId, agent, avatarSrc }: Props) {
  async function handleRetry() {
    const client = await getShipClient();
    await client.retryAgent(sessionId, agent.role);
  }

  const contextPct = agent.context_remaining_percent;
  const normalizedContextPct = contextPct === null ? null : Math.max(0, Math.min(contextPct, 100));
  const contextLow = normalizedContextPct !== null && normalizedContextPct < 20;
  const showContextIndicator =
    normalizedContextPct !== null && agent.state.tag !== "ContextExhausted";

  return (
    <Box className={agentHeader}>
      <Flex className={agentHeaderMain}>
        {avatarSrc ? (
          <img src={avatarSrc} alt={agent.role.tag} className={agentHeaderAvatar} />
        ) : (
          <Box className={agentHeaderAvatarFallback}>
            <AgentKindIcon kind={agent.kind} />
          </Box>
        )}
        <Flex className={agentHeaderBody}>
          <Flex className={agentHeaderSummaryRow}>
            <Text size="2" weight="medium" className={agentHeaderRole}>
              {agent.role.tag}
            </Text>
            {/* r[ui.agent-header.context-bar] */}
            {showContextIndicator && normalizedContextPct !== null && (
              <Box
                className={agentHeaderContext}
                data-tone={contextLow ? "low" : "normal"}
                role="progressbar"
                aria-label={`${agent.role.tag} context remaining`}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={normalizedContextPct}
              >
                <svg
                  viewBox="0 0 24 24"
                  className={agentHeaderContextSvg}
                  aria-hidden="true"
                  focusable="false"
                >
                  <circle cx="12" cy="12" r="9" className={agentHeaderContextTrack} />
                  <circle
                    cx="12"
                    cy="12"
                    r="9"
                    pathLength="100"
                    strokeDasharray="100"
                    strokeDashoffset={100 - normalizedContextPct}
                    className={agentHeaderContextArc}
                  />
                </svg>
              </Box>
            )}
          </Flex>
          <AgentModelPicker sessionId={sessionId} agent={agent} />
          <AgentEffortPicker sessionId={sessionId} agent={agent} />
        </Flex>
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
          <Button
            size="1"
            color="red"
            variant="soft"
            mt="2"
            onClick={() => {
              void handleRetry();
            }}
          >
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
          <Button
            size="1"
            color="red"
            variant="soft"
            mt="2"
            onClick={() => {
              void handleRetry();
            }}
          >
            <ArrowsClockwise size={12} />
            Retry Agent
          </Button>
        </Callout.Root>
      )}
    </Box>
  );
}
