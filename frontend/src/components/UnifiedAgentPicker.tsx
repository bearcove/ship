import { useEffect, useId, useState, useMemo } from "react";
import { Popover, TextField, Flex, Text, Box } from "@radix-ui/themes";
import type { AgentPreset, AgentSnapshot } from "../generated/ship";
import { AgentKindIcon } from "./AgentKindIcon";
import {
  agentHeaderControlRow,
  agentHeaderModelPickerContent,
  agentHeaderModelPickerList,
  agentHeaderModelPickerOption,
  agentHeaderPickerPrimary,
  agentHeaderPickerSecondary,
  agentHeaderPickerSummary,
  agentHeaderPickerSummaryText,
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

const LOGO_ASSETS: Record<string, string> = {
  anthropic: "/logos/anthropic.svg",
  google: "/logos/google.svg",
  minimax: "/logos/minimax.png",
  moonshot: "/logos/moonshot.svg",
  "z-ai": "/logos/z-ai.svg",
};

function PresetIcon({ preset }: { preset: AgentPreset }) {
  const logoSrc = preset.logo ? (LOGO_ASSETS[preset.logo] ?? null) : null;
  if (logoSrc) {
    return (
      <img
        src={logoSrc}
        style={{ width: 16, height: 16, objectFit: "contain", flexShrink: 0 }}
        alt=""
      />
    );
  }
  return <AgentKindIcon kind={preset.kind} />;
}

function renderPickerSummary(
  label: string,
  modelId: string | null,
  interactive: boolean,
  currentPreset: AgentPreset | null,
) {
  const primaryClassName = interactive ? agentHeaderPickerText : agentHeaderPickerPrimary;
  const showModel = modelId !== null && modelId !== label;
  return (
    <span className={agentHeaderPickerSummary}>
      {currentPreset && <PresetIcon preset={currentPreset} />}
      <span className={agentHeaderPickerSummaryText}>
        <Text size="2" color="gray" className={primaryClassName}>
          {label}
        </Text>
        {showModel && (
          <Text size="1" color="gray" className={agentHeaderPickerSecondary}>
            {modelId}
          </Text>
        )}
      </span>
    </span>
  );
}

export function UnifiedAgentPicker({
  label = "",
  selectedPresetId,
  presets,
  onSelectPreset,
  inference = null,
  fallbackLabel = "Select preset…",
  fallbackModelId = null,
  canSelect = true,
  error = null,
}: {
  label?: string;
  selectedPresetId: string | null;
  presets: AgentPreset[];
  onSelectPreset: (preset: AgentPreset) => void;
  inference?: PresetInference | null;
  fallbackLabel?: string;
  fallbackModelId?: string | null;
  canSelect?: boolean;
  error?: string | null;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const listboxId = useId();

  const currentPreset = findCurrentPreset(presets, selectedPresetId, inference);
  const currentLabel = currentPreset?.label ?? fallbackLabel;
  const currentModelId = currentPreset?.model_id ?? fallbackModelId;

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
  }, [selectedPresetId, inference?.kind.tag, inference?.modelId, inference?.provider]);

  function closePicker() {
    setOpen(false);
    setQuery("");
  }

  function handleSelectPreset(preset: AgentPreset) {
    onSelectPreset(preset);
    closePicker();
  }

  const hasPresets = presets.length > 0;
  const showLabel = label.length > 0;

  const pickerContent = hasPresets ? (
    canSelect ? (
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
            aria-label={label ? `Select ${label}` : "Select preset"}
          >
            {renderPickerSummary(currentLabel, currentModelId, true, currentPreset)}
          </button>
        </Popover.Trigger>
        <Popover.Content align="start" sideOffset={4} className={agentHeaderModelPickerContent}>
          <Flex direction="column" gap="2">
            <TextField.Root
              aria-label={label ? `Search ${label}` : "Search presets"}
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
                  handleSelectPreset(filteredPresets[0]!);
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
                      handleSelectPreset(preset);
                    }}
                  >
                    <span className={agentHeaderPresetOptionSummary}>
                      <Flex align="center" gap="2">
                        <PresetIcon preset={preset} />
                        <Text size="2">{preset.label}</Text>
                      </Flex>
                      {preset.model_id !== preset.label && (
                        <Text size="1" color="gray" className={agentHeaderPickerSecondary}>
                          {preset.model_id}
                        </Text>
                      )}
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
      <Flex className={agentHeaderControlRow}>
        {renderPickerSummary(currentLabel, currentModelId, false, currentPreset)}
      </Flex>
    )
  ) : (
    <Box px="1" py="1">
      <Text size="2" color="gray">
        No presets available
      </Text>
    </Box>
  );

  return (
    <>
      {showLabel ? (
        <Flex direction="column" gap="1">
          <Text size="2" weight="medium">
            {label}
          </Text>
          {pickerContent}
        </Flex>
      ) : (
        <Flex className={agentHeaderControlRow}>{pickerContent}</Flex>
      )}
      {error && (
        <Text size="2" color="red">
          {error}
        </Text>
      )}
    </>
  );
}
