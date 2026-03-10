import { useMemo } from "react";
import { Box, Button, Callout, DropdownMenu, Flex, Progress, Text } from "@radix-ui/themes";
import { ArrowsClockwise, Warning } from "@phosphor-icons/react";
import type { AgentSnapshot } from "../generated/ship";
import { AgentKindIcon } from "./AgentKindIcon";
import { getShipClient } from "../api/client";
import { agentHeader, agentHeaderRow } from "../styles/session-view.css";

interface Props {
  sessionId: string;
  agent: AgentSnapshot;
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
export function AgentHeader({ sessionId, agent }: Props) {
  const contextPct = agent.context_remaining_percent;
  const contextLow = contextPct !== null && contextPct < 20;

  const parsed = useMemo(() => {
    const all = agent.available_models.map(parseModelId);
    const models = [...new Set(all.map((m) => m.model))];
    const efforts = [...new Set(all.filter((m) => m.effort !== null).map((m) => m.effort!))];
    const current = agent.model_id ? parseModelId(agent.model_id) : null;
    const hasSplit = efforts.length > 0 && models.length > 0;
    return { models, efforts, current, hasSplit };
  }, [agent.model_id, agent.available_models]);

  async function handleSelectModel(modelId: string) {
    const client = await getShipClient();
    await client.setAgentModel(sessionId, agent.role, modelId);
  }

  function handleSelectModelName(model: string) {
    const effort = parsed.current?.effort ?? parsed.efforts[0] ?? null;
    void handleSelectModel(buildModelId(model, effort));
  }

  function handleSelectEffort(effort: string) {
    const model = parsed.current?.model ?? parsed.models[0];
    void handleSelectModel(buildModelId(model, effort));
  }

  return (
    <Box className={agentHeader}>
      <Flex className={agentHeaderRow}>
        <AgentKindIcon kind={agent.kind} />
        <Text size="2" weight="medium">
          {agent.role.tag}
        </Text>
        {agent.model_id !== null && agent.available_models.length > 1 && parsed.hasSplit ? (
          <Flex gap="1" align="center">
            <DropdownMenu.Root>
              <DropdownMenu.Trigger>
                <Text
                  size="1"
                  color="gray"
                  style={{ cursor: "pointer", textDecoration: "underline dotted" }}
                >
                  {parsed.current?.model ?? agent.model_id}
                </Text>
              </DropdownMenu.Trigger>
              <DropdownMenu.Content size="1">
                {parsed.models.map((model) => (
                  <DropdownMenu.Item
                    key={model}
                    onSelect={() => handleSelectModelName(model)}
                    style={model === parsed.current?.model ? { fontWeight: "bold" } : undefined}
                  >
                    {model}
                  </DropdownMenu.Item>
                ))}
              </DropdownMenu.Content>
            </DropdownMenu.Root>
            {parsed.current?.effort && (
              <>
                <Text size="1" color="gray">
                  /
                </Text>
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    <Text
                      size="1"
                      color="gray"
                      style={{ cursor: "pointer", textDecoration: "underline dotted" }}
                    >
                      {parsed.current.effort}
                    </Text>
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content size="1">
                    {parsed.efforts.map((effort) => (
                      <DropdownMenu.Item
                        key={effort}
                        onSelect={() => handleSelectEffort(effort)}
                        style={
                          effort === parsed.current?.effort ? { fontWeight: "bold" } : undefined
                        }
                      >
                        {effort}
                      </DropdownMenu.Item>
                    ))}
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              </>
            )}
          </Flex>
        ) : agent.model_id !== null && agent.available_models.length > 1 ? (
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              <Text
                size="1"
                color="gray"
                style={{ cursor: "pointer", textDecoration: "underline dotted" }}
              >
                {agent.model_id}
              </Text>
            </DropdownMenu.Trigger>
            <DropdownMenu.Content size="1">
              {agent.available_models.map((modelId) => (
                <DropdownMenu.Item
                  key={modelId}
                  onSelect={() => handleSelectModel(modelId)}
                  style={modelId === agent.model_id ? { fontWeight: "bold" } : undefined}
                >
                  {modelId}
                </DropdownMenu.Item>
              ))}
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        ) : agent.model_id !== null ? (
          <Text size="1" color="gray">
            {agent.model_id}
          </Text>
        ) : null}
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
