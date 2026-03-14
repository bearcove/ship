import { useEffect, useState } from "react";
import type { AgentPreset, AgentSnapshot } from "../generated/ship";
import { getShipClient } from "../api/client";
import { useAgentPresets } from "../hooks/useAgentPresets";
import { AgentPresetSelector } from "./AgentPresetSelector";

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

export function AgentPresetPicker({
  sessionId,
  agent,
}: {
  sessionId: string;
  agent: AgentSnapshot;
}) {
  const { presets, error: loadError, loading } = useAgentPresets();
  const [error, setError] = useState<string | null>(null);
  const [pendingPresetId, setPendingPresetId] = useState<string | null>(null);

  const activePreset = findActivePreset(agent, presets);
  const currentPresetId = pendingPresetId ?? activePreset?.id ?? agent.preset_id;
  const currentPreset = currentPresetId
    ? presets.find((preset) => preset.id === currentPresetId) ?? null
    : null;

  const canSwitchPresets =
    !loading &&
    loadError === null &&
    (presets.length > 1 || (presets.length === 1 && presets[0]?.id !== currentPresetId));

  useEffect(() => {
    setPendingPresetId(null);
    setError(null);
  }, [agent.kind.tag, agent.model_id, agent.preset_id, agent.provider]);

  async function handleSelectPreset(preset: AgentPreset) {
    if (preset.id === currentPresetId) {
      setError(null);
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

  if (agent.model_id === null && activePreset === null && currentPreset === null) {
    return null;
  }

  return (
    <AgentPresetSelector
      presets={presets}
      selectedPresetId={currentPresetId ?? null}
      inference={{
        kind: agent.kind,
        provider: agent.provider,
        modelId: agent.model_id,
      }}
      fallbackLabel={currentPreset?.label ?? agent.model_id ?? "Preset unavailable"}
      fallbackModelId={currentPreset?.model_id ?? agent.model_id}
      canSelect={canSwitchPresets}
      error={error ?? loadError}
      onSelectPreset={(preset) => {
        void handleSelectPreset(preset);
      }}
    />
  );
}
