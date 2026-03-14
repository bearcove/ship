import { useEffect, useId, useMemo, useState } from "react";
import { Popover, TextField, Flex, Text, Box } from "@radix-ui/themes";
import type { AgentSnapshot } from "../generated/ship";
import { getShipClient } from "../api/client";
import {
  agentHeaderControlRow,
  agentHeaderModelPickerContent,
  agentHeaderModelPickerList,
  agentHeaderModelPickerOption,
  agentHeaderModelPickerOptionText,
  agentHeaderPickerStatic,
  agentHeaderPickerText,
  agentHeaderPickerTextGrow,
  agentHeaderPickerTrigger,
} from "../styles/session-view.css";

export function useModelPicker(sessionId: string, agent: AgentSnapshot) {
  const [error, setError] = useState<string | null>(null);

  const availableModels = useMemo(
    () => Array.from(new Set(agent.available_models)),
    [agent.available_models],
  );

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

  return { availableModels, error, handleSelectModel };
}

export function AgentModelPicker({
  sessionId,
  agent,
}: {
  sessionId: string;
  agent: AgentSnapshot;
}) {
  const { availableModels, error, handleSelectModel } = useModelPicker(sessionId, agent);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const listboxId = useId();

  const normalizedQuery = query.trim().toLowerCase();
  const filteredModels = availableModels.filter((modelId) =>
    modelId.toLowerCase().includes(normalizedQuery),
  );

  function closePicker() {
    setOpen(false);
    setQuery("");
  }

  useEffect(() => {
    setOpen(false);
    setQuery("");
  }, [agent.model_id]);

  if (agent.model_id === null) return null;

  if (availableModels.length <= 1) {
    return (
      <>
        <Flex className={agentHeaderControlRow}>
          <Text size="2" color="gray" className={agentHeaderPickerStatic}>
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

  return (
    <>
      <Flex className={agentHeaderControlRow}>
        <Popover.Root
          open={open}
          onOpenChange={(nextOpen) => {
            setOpen(nextOpen);
            if (!nextOpen) setQuery("");
          }}
        >
          <Popover.Trigger>
            <button
              type="button"
              className={`${agentHeaderPickerTrigger} ${agentHeaderPickerTextGrow}`}
              aria-label="Select model"
            >
              <Text size="2" color="gray" className={agentHeaderPickerText}>
                {agent.model_id}
              </Text>
            </button>
          </Popover.Trigger>
          <Popover.Content align="start" sideOffset={4} className={agentHeaderModelPickerContent}>
            <Flex direction="column" gap="2">
              <TextField.Root
                aria-label="Search models"
                role="combobox"
                aria-expanded={open}
                aria-controls={listboxId}
                aria-autocomplete="list"
                placeholder="Search models…"
                value={query}
                autoFocus
                onChange={(event) => {
                  setQuery(event.target.value);
                  setOpen(true);
                }}
                onFocus={() => setOpen(true)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    closePicker();
                    return;
                  }

                  if (event.key === "Enter" && filteredModels.length > 0) {
                    event.preventDefault();
                    void handleSelectModel(filteredModels[0]!);
                    closePicker();
                  }
                }}
              />
              <Box id={listboxId} role="listbox" className={agentHeaderModelPickerList}>
                {filteredModels.length > 0 ? (
                  filteredModels.map((modelId) => (
                    <Box
                      key={modelId}
                      role="option"
                      aria-selected={modelId === agent.model_id}
                      data-selected={modelId === agent.model_id ? "true" : "false"}
                      className={agentHeaderModelPickerOption}
                      onMouseDown={(event) => {
                        event.preventDefault();
                        void handleSelectModel(modelId);
                        closePicker();
                      }}
                    >
                      <Text size="2" className={agentHeaderModelPickerOptionText}>
                        {modelId}
                      </Text>
                    </Box>
                  ))
                ) : (
                  <Box px="3" py="2">
                    <Text size="2" color="gray">
                      No matching models
                    </Text>
                  </Box>
                )}
              </Box>
            </Flex>
          </Popover.Content>
        </Popover.Root>
      </Flex>
      {error && (
        <Text size="2" color="red">
          {error}
        </Text>
      )}
    </>
  );
}
