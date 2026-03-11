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
  const { effort_config_id, effort_value_id, available_effort_values } = agent;

  if (!effort_config_id || !effort_value_id) return null;

  const currentEffort = available_effort_values.find((e) => e.id === effort_value_id);

  async function handleSelect(valueId: string) {
    const client = await getShipClient();
    await client.setAgentEffort(sessionId, agent.role, effort_config_id!, valueId);
  }

  if (available_effort_values.length <= 1) {
    return (
      <Flex className={agentHeaderControlRow}>
        <Text size="1" color="gray" className={agentHeaderPickerStatic}>
          {currentEffort?.name ?? effort_value_id}
        </Text>
      </Flex>
    );
  }

  return (
    <Flex className={agentHeaderControlRow}>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger
          className={`${agentHeaderPickerTrigger} ${agentHeaderPickerTextGrow}`}
        >
          <Text size="1" color="gray" className={agentHeaderPickerText}>
            {currentEffort?.name ?? effort_value_id}
          </Text>
        </DropdownMenu.Trigger>
        <DropdownMenu.Content size="1">
          {available_effort_values.map((ev) => (
            <DropdownMenu.Item
              key={ev.id}
              onSelect={() => void handleSelect(ev.id)}
              style={ev.id === effort_value_id ? { fontWeight: "bold" } : undefined}
            >
              {ev.name}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </Flex>
  );
}
