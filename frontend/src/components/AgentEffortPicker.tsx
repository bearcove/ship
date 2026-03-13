import { useEffect, useState } from "react";
import { DropdownMenu, Flex, Text } from "@radix-ui/themes";
import type { AgentSnapshot } from "../generated/ship";
import { getShipClient } from "../api/client";
import {
  agentHeaderControlRow,
  agentHeaderPickerStatic,
  agentHeaderPickerText,
  agentHeaderPickerTextGrow,
  agentHeaderPickerTrigger,
} from "../styles/session-view.css";

export function AgentEffortPicker({
  sessionId,
  agent,
}: {
  sessionId: string;
  agent: AgentSnapshot;
}) {
  const [error, setError] = useState<string | null>(null);
  const [selectedEffortId, setSelectedEffortId] = useState<string | null>(agent.effort_value_id);
  const { effort_config_id, effort_value_id, available_effort_values } = agent;

  useEffect(() => {
    setSelectedEffortId(effort_value_id);
  }, [effort_value_id]);

  if (!effort_config_id || !effort_value_id) return null;

  const currentConfigId = effort_config_id;
  const currentEffortId = selectedEffortId ?? effort_value_id;
  const currentEffort = available_effort_values.find((effort) => effort.id === currentEffortId);

  async function handleSelect(valueId: string) {
    if (valueId === currentEffortId) return;

    const previousEffortId = currentEffortId;
    setSelectedEffortId(valueId);

    try {
      const client = await getShipClient();
      const result = await client.setAgentEffort(sessionId, agent.role, currentConfigId, valueId);
      if (result.tag === "AgentNotSpawned") {
        setSelectedEffortId(previousEffortId);
        setError("Agent not running");
        return;
      }
      if (result.tag === "SessionNotFound") {
        setSelectedEffortId(previousEffortId);
        setError("Session not found");
        return;
      }
      if (result.tag === "Failed") {
        setSelectedEffortId(previousEffortId);
        setError(result.message);
        return;
      }
      if (result.tag === "Ok") {
        setError(null);
      }
    } catch (error) {
      setSelectedEffortId(previousEffortId);
      setError(error instanceof Error ? error.message : "Failed to update effort");
    }
  }

  if (available_effort_values.length <= 1) {
    return (
      <>
        <Flex className={agentHeaderControlRow}>
          <Text size="1" color="gray" className={agentHeaderPickerStatic}>
            {currentEffort?.name ?? currentEffortId}
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

  return (
    <>
      <Flex className={agentHeaderControlRow}>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger
            className={`${agentHeaderPickerTrigger} ${agentHeaderPickerTextGrow}`}
          >
            <Text size="1" color="gray" className={agentHeaderPickerText}>
              {currentEffort?.name ?? currentEffortId}
            </Text>
          </DropdownMenu.Trigger>
          <DropdownMenu.Content size="1">
            {available_effort_values.map((effort) => (
              <DropdownMenu.Item
                key={effort.id}
                onSelect={() => void handleSelect(effort.id)}
                style={effort.id === currentEffortId ? { fontWeight: "bold" } : undefined}
              >
                {effort.name}
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
