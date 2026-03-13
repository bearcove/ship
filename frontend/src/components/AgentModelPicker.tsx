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
    const models = [...new Set(all.map((model) => model.model))];
    const efforts = [
      ...new Set(all.flatMap((model) => (model.effort === null ? [] : [model.effort]))),
    ];
    const current = agent.model_id ? parseModelId(agent.model_id) : null;
    const hasDedicatedEffort = agent.effort_config_id !== null && agent.effort_value_id !== null;
    const requiresEffortSuffix = efforts.length > 0;
    const hasSplitModelPicker = !hasDedicatedEffort && requiresEffortSuffix && models.length > 0;
    return {
      current,
      models,
      efforts,
      hasDedicatedEffort,
      hasSplitModelPicker,
      requiresEffortSuffix,
    };
  }, [agent.available_models, agent.effort_config_id, agent.effort_value_id, agent.model_id]);

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

  function resolveModelId(model: string): string {
    const exactMatch = agent.available_models.find((modelId) => modelId === model);
    if (exactMatch) return exactMatch;

    const matchingModels = agent.available_models.filter(
      (modelId) => parseModelId(modelId).model === model,
    );
    if (matchingModels.length === 0) return model;
    if (!parsed.requiresEffortSuffix) return matchingModels[0]!;

    const preferredEffort = parsed.hasDedicatedEffort
      ? agent.effort_value_id
      : (parsed.current?.effort ?? parsed.efforts[0] ?? null);
    if (preferredEffort !== null) {
      const preferredModelId = buildModelId(model, preferredEffort);
      if (matchingModels.includes(preferredModelId)) return preferredModelId;
    }

    return matchingModels[0]!;
  }

  function handleSelectModelName(model: string) {
    void handleSelectModel(resolveModelId(model));
  }

  function handleSelectEffort(effort: string) {
    const model = parsed.current?.model ?? parsed.models[0];
    if (!model) return;
    void handleSelectModel(buildModelId(model, effort));
  }

  return { parsed, error, handleSelectModel, handleSelectModelName, handleSelectEffort };
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

  const currentModelLabel = parsed.current?.model ?? agent.model_id;
  const showModelDropdown = parsed.hasDedicatedEffort
    ? parsed.models.length > 1
    : agent.available_models.length > 1;

  if (agent.available_models.length > 1 && parsed.hasSplitModelPicker) {
    return (
      <>
        <Flex className={agentHeaderControlRow}>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger className={agentHeaderPickerTrigger}>
              <Text size="1" color="gray" className={agentHeaderPickerText}>
                {currentModelLabel}
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

  if (showModelDropdown) {
    return (
      <>
        <Flex className={agentHeaderControlRow}>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger
              className={`${agentHeaderPickerTrigger} ${agentHeaderPickerTextGrow}`}
            >
              <Text size="1" color="gray" className={agentHeaderPickerText}>
                {parsed.hasDedicatedEffort ? currentModelLabel : agent.model_id}
              </Text>
            </DropdownMenu.Trigger>
            <DropdownMenu.Content size="1">
              {parsed.hasDedicatedEffort
                ? parsed.models.map((model) => (
                    <DropdownMenu.Item
                      key={model}
                      onSelect={() => handleSelectModelName(model)}
                      style={model === parsed.current?.model ? { fontWeight: "bold" } : undefined}
                    >
                      {model}
                    </DropdownMenu.Item>
                  ))
                : agent.available_models.map((modelId) => (
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
          {parsed.hasDedicatedEffort ? currentModelLabel : agent.model_id}
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
