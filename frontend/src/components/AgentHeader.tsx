import { Box, Button, Callout, Flex, Text } from "@radix-ui/themes";
import { ArrowsClockwise, Warning } from "@phosphor-icons/react";
import type { AgentSnapshot } from "../generated/ship";
import { AgentKindIcon } from "./AgentKindIcon";
import { AgentModelPicker } from "./AgentModelPicker";
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

function parseModelId(modelId: string): { model: string; effort: string | null } {
  const slashIndex = modelId.lastIndexOf("/");
  if (slashIndex === -1) return { model: modelId, effort: null };
  return { model: modelId.slice(0, slashIndex), effort: modelId.slice(slashIndex + 1) };
}

function buildModelId(model: string, effort: string | null): string {
  return effort ? `${model}/${effort}` : model;
}

// r[ui.agent-header.layout]
// r[view.agent-panel.state]
export function AgentHeader({ sessionId, agent, avatarSrc }: Props) {
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
