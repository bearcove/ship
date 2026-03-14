import { useEffect, useId, useState, useMemo } from "react";
import { Popover, TextField, Flex, Text, Box } from "@radix-ui/themes";
import type { AgentPreset } from "../generated/ship";
import {
  agentHeaderModelPickerContent,
  agentHeaderModelPickerList,
  agentHeaderModelPickerOption,
  agentHeaderPickerPrimary,
  agentHeaderPickerSummary,
  agentHeaderPickerText,
  agentHeaderPickerTextGrow,
  agentHeaderPickerTrigger,
} from "../styles/session-view.css";

function renderPickerSummary(
  label: string,
  interactive: boolean,
) {
  const primaryClassName = interactive ? agentHeaderPickerText : agentHeaderPickerPrimary;
  return (
    <span className={agentHeaderPickerSummary}>
      <Text size="2" color="gray" className={primaryClassName}>
        {label}
      </Text>
    </span>
  );
}

export function UnifiedAgentPicker({
  label,
  selectedPresetId,
  presets,
  onSelectPreset,
}: {
  label: string;
  selectedPresetId: string | null;
  presets: AgentPreset[];
  onSelectPreset: (preset: AgentPreset) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const listboxId = useId();

  const selectedPreset = selectedPresetId
    ? presets.find((p) => p.id === selectedPresetId) ?? null
    : null;

  const currentLabel = selectedPreset?.label ?? "Select preset…";

  const normalizedQuery = query.trim().toLowerCase();

  const filteredPresets = useMemo(
    () =>
      presets.filter((preset) => {
        if (normalizedQuery.length === 0) {
          return true;
        }
        return (
          [preset.label, preset.model_id, preset.provider, preset.kind.tag]
            .join(" ")
            .toLowerCase()
            .includes(normalizedQuery)
        );
      }),
    [presets, normalizedQuery],
  );

  useEffect(() => {
    setOpen(false);
    setQuery("");
  }, [selectedPresetId]);

  function closePicker() {
    setOpen(false);
    setQuery("");
  }

  function handleSelectPreset(preset: AgentPreset) {
    onSelectPreset(preset);
    closePicker();
  }

  const hasPresets = presets.length > 0;

  return (
    <>
      <Flex direction="column" gap="1">
        <Text size="2" weight="medium">
          {label}
        </Text>
        {hasPresets ? (
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
                aria-label={`Select ${label}`}
              >
                {renderPickerSummary(currentLabel, true)}
              </button>
            </Popover.Trigger>
            <Popover.Content align="start" sideOffset={4} className={agentHeaderModelPickerContent}>
              <Flex direction="column" gap="2">
                <TextField.Root
                  aria-label={`Search ${label}`}
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
                    if (event.key === "Enter") {
                      if (filteredPresets.length > 0) {
                        event.preventDefault();
                        handleSelectPreset(filteredPresets[0]!);
                      }
                    }
                  }}
                />
                <Box id={listboxId} role="listbox" className={agentHeaderModelPickerList}>
                  {filteredPresets.length > 0 ? (
                    filteredPresets.map((preset) => (
                      <Box
                        key={preset.id}
                        role="option"
                        aria-selected={preset.id === selectedPreset?.id}
                        data-selected={preset.id === selectedPreset?.id ? "true" : "false"}
                        className={agentHeaderModelPickerOption}
                        onMouseDown={(event) => {
                          event.preventDefault();
                          handleSelectPreset(preset);
                        }}
                      >
                        <Text size="2">{preset.label}</Text>
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
          <Box px="1" py="1">
            <Text size="2" color="gray">
              No presets available
            </Text>
          </Box>
        )}
      </Flex>
    </>
  );
}
