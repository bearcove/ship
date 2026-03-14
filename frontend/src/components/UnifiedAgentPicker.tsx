import { useEffect, useId, useState, useMemo } from "react";
import { Popover, TextField, Flex, Text, Box, Badge } from "@radix-ui/themes";
import type { AgentKind, AgentPreset } from "../generated/ship";
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

type Availability = {
  claude: boolean;
  codex: boolean;
  opencode: boolean;
};

const AGENT_KINDS: Array<{ tag: "Claude" | "Codex" | "OpenCode"; label: string; color: "violet" | "cyan" | "green" }> = [
  { tag: "Claude", label: "Claude", color: "violet" },
  { tag: "Codex", label: "Codex", color: "cyan" },
  { tag: "OpenCode", label: "OpenCode", color: "green" },
];

function renderPickerSummary(
  label: string,
  modelId: string | null,
  interactive: boolean,
) {
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

function isAgentKindAvailable(
  kind: AgentKind,
  availability: Availability,
) {
  if (kind.tag === "Claude") return availability.claude;
  if (kind.tag === "Codex") return availability.codex;
  return availability.opencode;
}

function kindToTag(kind: AgentKind): "Claude" | "Codex" | "OpenCode" {
  return kind.tag;
}

export function UnifiedAgentPicker({
  label,
  selectedKind,
  selectedPresetId,
  selectedModelId,
  presets,
  availability,
  onSelectKind,
  onSelectPreset,
}: {
  label: string;
  selectedKind: AgentKind;
  selectedPresetId: string | null;
  selectedModelId: string | null;
  presets: AgentPreset[];
  availability: Availability;
  onSelectKind: (kind: AgentKind) => void;
  onSelectPreset: (preset: AgentPreset) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState<"presets" | "kinds">(
    presets.length > 0 ? "presets" : "kinds",
  );
  const listboxId = useId();

  const selectedPreset = selectedPresetId
    ? presets.find((p) => p.id === selectedPresetId) ?? null
    : null;

  const currentLabel = selectedPreset?.label ?? selectedKind.tag;
  const currentModelId = selectedPreset?.model_id ?? selectedModelId;

  const availableKinds = useMemo(
    () => AGENT_KINDS.filter((k) => availability[k.tag.toLowerCase() as keyof Availability]),
    [availability],
  );

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

  const filteredKinds = useMemo(
    () =>
      availableKinds.filter((k) => {
        if (normalizedQuery.length === 0) {
          return true;
        }
        return k.label.toLowerCase().includes(normalizedQuery);
      }),
    [availableKinds, normalizedQuery],
  );

  useEffect(() => {
    setOpen(false);
    setQuery("");
  }, [selectedPresetId, selectedKind, selectedModelId]);

  useEffect(() => {
    if (presets.length > 0 && activeTab === "kinds") {
      setActiveTab("presets");
    } else if (presets.length === 0 && activeTab === "presets") {
      setActiveTab("kinds");
    }
  }, [presets.length, activeTab]);

  function closePicker() {
    setOpen(false);
    setQuery("");
  }

  function handleSelectPreset(preset: AgentPreset) {
    onSelectPreset(preset);
    closePicker();
  }

  function handleSelectKind(kind: { tag: "Claude" | "Codex" | "OpenCode" }) {
    onSelectKind({ tag: kind.tag });
    closePicker();
  }

  const hasPresets = presets.length > 0;
  const canSelect = hasPresets || availableKinds.length > 0;

  return (
    <>
      <Flex direction="column" gap="1">
        <Text size="2" weight="medium">
          {label}
        </Text>
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
                aria-label={`Select ${label}`}
              >
                {renderPickerSummary(currentLabel, currentModelId, true)}
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
                  placeholder={`Search ${hasPresets ? "presets or agents" : "agents"}…`}
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
                      if (activeTab === "presets" && filteredPresets.length > 0) {
                        event.preventDefault();
                        handleSelectPreset(filteredPresets[0]!);
                      } else if (activeTab === "kinds" && filteredKinds.length > 0) {
                        event.preventDefault();
                        handleSelectKind(filteredKinds[0]!);
                      }
                    }
                  }}
                />
                {hasPresets && (
                  <Flex gap="1">
                    <button
                      type="button"
                      style={{
                        padding: "4px 8px",
                        borderRadius: "var(--radius-2)",
                        border: "none",
                        background: activeTab === "presets" ? "var(--gray-5)" : "transparent",
                        cursor: "pointer",
                        fontSize: "var(--font-size-1)",
                      }}
                      onClick={() => setActiveTab("presets")}
                    >
                      Presets ({filteredPresets.length})
                    </button>
                    <button
                      type="button"
                      style={{
                        padding: "4px 8px",
                        borderRadius: "var(--radius-2)",
                        border: "none",
                        background: activeTab === "kinds" ? "var(--gray-5)" : "transparent",
                        cursor: "pointer",
                        fontSize: "var(--font-size-1)",
                      }}
                      onClick={() => setActiveTab("kinds")}
                    >
                      Agents ({filteredKinds.length})
                    </button>
                  </Flex>
                )}
                <Box id={listboxId} role="listbox" className={agentHeaderModelPickerList}>
                  {activeTab === "presets" ? (
                    filteredPresets.length > 0 ? (
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
                    )
                  ) : filteredKinds.length > 0 ? (
                    filteredKinds.map((kind) => {
                      const isSelected = selectedKind.tag === kind.tag && selectedPresetId === null;
                      return (
                        <Box
                          key={kind.tag}
                          role="option"
                          aria-selected={isSelected}
                          data-selected={isSelected ? "true" : "false"}
                          className={agentHeaderModelPickerOption}
                          onMouseDown={(event) => {
                            event.preventDefault();
                            handleSelectKind(kind);
                          }}
                        >
                          <Flex align="center" gap="2">
                            <Badge color={kind.color} variant="soft" size="1">
                              {kind.label}
                            </Badge>
                            <Text size="1" color="gray">
                              Raw agent
                            </Text>
                          </Flex>
                        </Box>
                      );
                    })
                  ) : (
                    <Box px="3" py="2">
                      <Text size="2" color="gray">
                        No matching agents
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
    </>
  );
}
