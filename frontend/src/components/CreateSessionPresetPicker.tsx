import { useAgentPresets } from "../hooks/useAgentPresets";
import type { AgentKind, AgentPreset } from "../generated/ship";
import { UnifiedAgentPicker } from "./UnifiedAgentPicker";

export function CreateSessionPresetPicker({
  selectedPresetId,
  selectedKind,
  selectedModelId,
  onSelectPreset,
}: {
  selectedPresetId: string | null;
  selectedKind: AgentKind;
  selectedModelId: string | null;
  onSelectPreset: (preset: AgentPreset) => void;
}) {
  const { presets, loading, error } = useAgentPresets();
  const currentPreset = selectedPresetId
    ? presets.find((preset) => preset.id === selectedPresetId) ?? null
    : null;

  const canSelect = !loading && error === null && presets.length > 0;

  return (
    <UnifiedAgentPicker
      presets={presets}
      selectedPresetId={selectedPresetId}
      inference={{ kind: selectedKind, provider: null, modelId: selectedModelId }}
      fallbackLabel={currentPreset?.label ?? selectedKind.tag}
      fallbackModelId={currentPreset?.model_id ?? selectedModelId}
      canSelect={canSelect}
      error={error}
      onSelectPreset={onSelectPreset}
    />
  );
}
