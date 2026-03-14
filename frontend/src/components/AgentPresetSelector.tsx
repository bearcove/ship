import { useEffect, useId, useState } from "react";
import { Popover, TextField, Flex, Text, Box } from "@radix-ui/themes";
import type { AgentPreset, AgentSnapshot } from "../generated/ship";
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

type PresetInference = {
  kind: AgentSnapshot["kind"];
  provider: string | null;
  modelId: string | null;
};

function sameAgentKind(left: AgentSnapshot["kind"], right: AgentPreset["kind"]) {
  return left.tag === right.tag;
}

function findCurrentPreset(
  presets: AgentPreset[],
  selectedPresetId: string | null,
  inference: PresetInference | null,
) {
  if (selectedPresetId !== null) {
    return presets.find((preset) => preset.id === selectedPresetId) ?? null;
  }
  if (inference === null || inference.modelId === null) {
    return null;
  }
  return (
    presets.find((preset) => {
      if (!sameAgentKind(inference.kind, preset.kind)) {
        return false;
      }
      if (preset.model_id !== inference.modelId) {
        return false;
      }
      if (inference.provider !== null && preset.provider !== inference.provider) {
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

export function AgentPresetSelector({
  presets,
  selectedPresetId,
  inference,
  fallbackLabel,
  fallbackModelId,
  canSelect,
  error,
  onSelectPreset,
}: {
  presets: AgentPreset[];
  selectedPresetId: string | null;
  inference: PresetInference | null;
  fallbackLabel: string;
  fallbackModelId: string | null;
  canSelect: boolean;
  error: string | null;
  onSelectPreset: (preset: AgentPreset) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const listboxId = useId();
  const currentPreset = findCurrentPreset(presets, selectedPresetId, inference);
  const currentLabel = currentPreset?.label ?? fallbackLabel;
  const currentModelId = currentPreset?.model_id ?? fallbackModelId;

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

  useEffect(() => {
    setOpen(false);
    setQuery("");
  }, [selectedPresetId, inference?.kind.tag, inference?.modelId, inference?.provider]);

  function closePicker() {
    setOpen(false);
    setQuery("");
  }

  return (
    <>
      <Flex className={agentHeaderControlRow}>
        {canSelect ? (
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
                      onSelectPreset(filteredPresets[0]!);
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
                        aria-selected={preset.id === currentPreset?.id}
                        data-selected={preset.id === currentPreset?.id ? "true" : "false"}
                        className={agentHeaderModelPickerOption}
                        onMouseDown={(event) => {
                          event.preventDefault();
                          onSelectPreset(preset);
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
      {error && (
        <Text size="2" color="red">
          {error}
        </Text>
      )}
    </>
  );
}
