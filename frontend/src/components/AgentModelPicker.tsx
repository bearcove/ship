import { useMemo, useState } from "react";
import { DropdownMenu, Flex, Text } from "@radix-ui/themes";
import type { AgentSnapshot } from "../generated/ship";
import { getShipClient } from "../api/client";
import {
  agentHeaderControlRow,
  agentHeaderPickerStatic,
  agentHeaderPickerText,
  agentHeaderPickerTextGrow,
  agentHeaderPickerTrigger,
  agentHeaderSlash,
} from "../styles/session-view.css";

function parseModelId(modelId: string): { model: string; effort: string | null } {
  const slashIndex = modelId.lastIndexOf("/");
  if (slashIndex === -1) return { model: modelId, effort: null };
  return { model: modelId.slice(0, slashIndex), effort: modelId.slice(slashIndex + 1) };
}

function buildModelId(model: string, effort: string | null): string {
  return effort ? `${model}/${effort}` : model;
}

export function useModelPicker(sessionId: string, agent: AgentSnapshot) {
  const [error, setError] = useState<string | null>(null);
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
    const result = await client.setAgentModel(sessionId, agent.role, modelId);
    if (result.tag === "AgentNotSpawned") {
      setError("Agent not running");
      return;
    }
    if (result.tag === "Failed") {
      setError(result.message);
      return;
    }
    if (result.tag === "Ok") {
      setError(null);
    }
  }

  function handleSelectModelName(model: string) {
    const effort = parsed.current?.effort ?? parsed.efforts[0] ?? null;
    void handleSelectModel(buildModelId(model, effort));
  }

  function handleSelectEffort(effort: string) {
    const model = parsed.current?.model ?? parsed.models[0];
    void handleSelectModel(buildModelId(model, effort));
  }

  return { parsed, error, setError, handleSelectModel, handleSelectModelName, handleSelectEffort };
}

export function AgentModelPicker({
  sessionId,
  agent,
}: {
  sessionId: string;
  agent: AgentSnapshot;
}) {
  const { parsed, error, handleSelectModel, handleSelectModelName, handleSelectEffort } =
    useModelPicker(sessionId, agent);

  if (agent.model_id === null) return null;

  if (agent.available_models.length > 1 && parsed.hasSplit) {
    return (
      <>
        <Flex className={agentHeaderControlRow}>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger className={agentHeaderPickerTrigger}>
              <Text size="1" color="gray" className={agentHeaderPickerText}>
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
              <Text size="1" color="gray" className={agentHeaderSlash}>
                /
              </Text>
              <DropdownMenu.Root>
                <DropdownMenu.Trigger className={agentHeaderPickerTrigger}>
                  <Text size="1" color="gray" className={agentHeaderPickerText}>
                    {parsed.current.effort}
                  </Text>
                </DropdownMenu.Trigger>
                <DropdownMenu.Content size="1">
                  {parsed.efforts.map((effort) => (
                    <DropdownMenu.Item
                      key={effort}
                      onSelect={() => handleSelectEffort(effort)}
                      style={effort === parsed.current?.effort ? { fontWeight: "bold" } : undefined}
                    >
                      {effort}
                    </DropdownMenu.Item>
                  ))}
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            </>
          )}
        </Flex>
        {error && (
          <Text size="1" color="red">
            {error}
          </Text>
        )}
      </>
    );
  }

  if (agent.available_models.length > 1) {
    return (
      <>
        <Flex className={agentHeaderControlRow}>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger
              className={`${agentHeaderPickerTrigger} ${agentHeaderPickerTextGrow}`}
            >
              <Text size="1" color="gray" className={agentHeaderPickerText}>
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
        </Flex>
        {error && (
          <Text size="1" color="red">
            {error}
          </Text>
        )}
      </>
    );
  }

  return (
    <>
      <Flex className={agentHeaderControlRow}>
        <Text size="1" color="gray" className={agentHeaderPickerStatic}>
          {agent.model_id}
        </Text>
      </Flex>
      {error && (
        <Text size="1" color="red">
          {error}
        </Text>
      )}
    </>
  );
}
