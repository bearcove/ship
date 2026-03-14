import { useEffect, useId, useState } from "react";
import { Popover, TextField, Flex, Text, Box } from "@radix-ui/themes";
import type { AgentPreset, AgentSnapshot } from "../generated/ship";
import { getShipClient } from "../api/client";
import { useAgentPresets } from "../hooks/useAgentPresets";
import {
  agentHeaderControlRow,
  agentHeaderModelPickerContent,
  agentHeaderModelPickerList,
  agentHeaderModelPickerOption,
  agentHeaderPickerPrimary,
  agentHeaderPickerSecondary,
  agentHeaderPickerSummary,
  agentHeaderPickerText,
  agentHeaderPickerTextGrow,
  agentHeaderPickerTrigger,
  agentHeaderPresetOptionSummary,
} from "../styles/session-view.css";

function sameAgentKind(left: AgentSnapshot["kind"], right: AgentPreset["kind"]) {
  return left.tag === right.tag;
}

function findActivePreset(agent: AgentSnapshot, presets: AgentPreset[]) {
  if (agent.preset_id !== null) {
    const activePreset = presets.find((preset) => preset.id === agent.preset_id);
    if (activePreset) {
      return activePreset;
    }
  }

  if (agent.model_id === null) {
    return null;
  }

  return (
    presets.find((preset) => {
      if (!sameAgentKind(agent.kind, preset.kind)) {
        return false;
      }
      if (preset.model_id !== agent.model_id) {
        return false;
      }
      if (agent.provider !== null && preset.provider !== agent.provider) {
        return false;
      }
      return true;
    }) ?? null
  );
}

function renderPickerSummary(label: string, modelId: string | null, interactive: boolean) {
  const primaryClassName = interactive ? agentHeaderPickerText : agentHeaderPickerPrimary;
  const showModel = modelId !== null && modelId !== label;

  return (
    <span className={agentHeaderPickerSummary}>
      <Text size="2" color="gray" className={primaryClassName}>
        {label}
      </Text>
      {showModel && (
        <Text size="1" color="gray" className={agentHeaderPickerSecondary}>
          {modelId}
        </Text>
      )}
    </span>
  );
}

export function AgentPresetPicker({
  sessionId,
  agent,
}: {
  sessionId: string;
  agent: AgentSnapshot;
}) {
  const { presets, error: loadError, loading } = useAgentPresets();
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [pendingPresetId, setPendingPresetId] = useState<string | null>(null);
  const listboxId = useId();

  const activePreset = findActivePreset(agent, presets);
  const pendingPreset = pendingPresetId
    ? presets.find((preset) => preset.id === pendingPresetId) ?? null
    : null;
  const displayPreset = pendingPreset ?? activePreset;
  const currentPresetId = pendingPresetId ?? activePreset?.id ?? agent.preset_id;
  const currentLabel = displayPreset?.label ?? agent.model_id ?? "Preset unavailable";
  const currentModelId = displayPreset?.model_id ?? agent.model_id;

  const normalizedQuery = query.trim().toLowerCase();
  const filteredPresets = presets.filter((preset) => {
    if (normalizedQuery.length === 0) {
      return true;
    }
    return [preset.label, preset.model_id, preset.provider, preset.kind.tag]
      .join(" ")
      .toLowerCase()
      .includes(normalizedQuery);
  });

  const canSwitchPresets =
    !loading &&
    loadError === null &&
    (presets.length > 1 || (presets.length === 1 && presets[0]?.id !== currentPresetId));

  useEffect(() => {
    setOpen(false);
    setQuery("");
    setPendingPresetId(null);
    setError(null);
  }, [agent.kind.tag, agent.model_id, agent.preset_id, agent.provider]);

  function closePicker() {
    setOpen(false);
    setQuery("");
  }

  async function handleSelectPreset(preset: AgentPreset) {
    if (preset.id === currentPresetId) {
      setError(null);
      closePicker();
      return;
    }

    setPendingPresetId(preset.id);

    try {
      const client = await getShipClient();
      const result = await client.setAgentPreset(sessionId, agent.role, preset.id);
      if (result.tag === "AgentNotSpawned") {
        setPendingPresetId(null);
        setError("Agent not running");
        return;
      }
      if (result.tag === "SessionNotFound") {
        setPendingPresetId(null);
        setError("Session not found");
        return;
      }
      if (result.tag === "PresetNotFound") {
        setPendingPresetId(null);
        setError("Preset not found");
        return;
      }
      if (result.tag === "Failed") {
        setPendingPresetId(null);
        setError(result.message);
        return;
      }
      if (result.tag === "Ok") {
        setError(null);
      }
    } catch (selectionError) {
      setPendingPresetId(null);
      setError(
        selectionError instanceof Error ? selectionError.message : "Failed to update preset",
      );
    }
  }

  if (agent.model_id === null && activePreset === null && pendingPreset === null) {
    return null;
  }

  return (
    <>
      <Flex className={agentHeaderControlRow}>
        {canSwitchPresets ? (
          <Popover.Root
            open={open}
            onOpenChange={(nextOpen) => {
              setOpen(nextOpen);
              if (!nextOpen) {
                setQuery("");
              }
            }}
          >
            <Popover.Trigger>
              <button
                type="button"
                className={`${agentHeaderPickerTrigger} ${agentHeaderPickerTextGrow}`}
                aria-label="Select preset"
              >
                {renderPickerSummary(currentLabel, currentModelId, true)}
              </button>
            </Popover.Trigger>
            <Popover.Content align="start" sideOffset={4} className={agentHeaderModelPickerContent}>
              <Flex direction="column" gap="2">
                <TextField.Root
                  aria-label="Search presets"
                  role="combobox"
                  aria-expanded={open}
                  aria-controls={listboxId}
                  aria-autocomplete="list"
                  placeholder="Search presets…"
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

                    if (event.key === "Enter" && filteredPresets.length > 0) {
                      event.preventDefault();
                      void handleSelectPreset(filteredPresets[0]!);
                      closePicker();
                    }
                  }}
                />
                <Box id={listboxId} role="listbox" className={agentHeaderModelPickerList}>
                  {filteredPresets.length > 0 ? (
                    filteredPresets.map((preset) => (
                      <Box
                        key={preset.id}
                        role="option"
                        aria-selected={preset.id === currentPresetId}
                        data-selected={preset.id === currentPresetId ? "true" : "false"}
                        className={agentHeaderModelPickerOption}
                        onMouseDown={(event) => {
                          event.preventDefault();
                          void handleSelectPreset(preset);
                          closePicker();
                        }}
                      >
                        <span className={agentHeaderPresetOptionSummary}>
                          <Text size="2">{preset.label}</Text>
                          <Text size="1" color="gray" className={agentHeaderPickerSecondary}>
                            {preset.model_id}
                          </Text>
                        </span>
                      </Box>
                    ))
                  ) : (
                    <Box px="3" py="2">
                      <Text size="2" color="gray">
                        No matching presets
                      </Text>
                    </Box>
                  )}
                </Box>
              </Flex>
            </Popover.Content>
          </Popover.Root>
        ) : (
          renderPickerSummary(currentLabel, currentModelId, false)
        )}
      </Flex>
      {(error ?? loadError) && (
        <Text size="2" color="red">
          {error ?? loadError}
        </Text>
      )}
    </>
  );
}
