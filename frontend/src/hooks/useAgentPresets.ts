import { useEffect, useState } from "react";
import type { AgentPreset } from "../generated/ship";
import { getShipClient, onClientReady } from "../api/client";

type AgentPresetsSnapshot = {
  presets: AgentPreset[];
  error: string | null;
  loading: boolean;
};

const listeners = new Set<() => void>();

let snapshot: AgentPresetsSnapshot = {
  presets: [],
  error: null,
  loading: false,
};
let hasLoaded = false;
let inflight: Promise<void> | null = null;

function notifyListeners() {
  for (const listener of listeners) {
    listener();
  }
}

async function loadAgentPresets() {
  if (inflight) {
    await inflight;
    return;
  }

  snapshot = { ...snapshot, loading: true, error: null };
  notifyListeners();

  inflight = (async () => {
    try {
      const client = await getShipClient();
      const presets = await client.listAgentPresets();
      snapshot = {
        presets,
        error: null,
        loading: false,
      };
      hasLoaded = true;
    } catch (error) {
      snapshot = {
        presets: [],
        error: error instanceof Error ? error.message : "Failed to load presets",
        loading: false,
      };
    } finally {
      inflight = null;
      notifyListeners();
    }
  })();

  await inflight;
}

export function useAgentPresets() {
  const [state, setState] = useState(snapshot);

  useEffect(() => {
    function update() {
      setState(snapshot);
    }

    listeners.add(update);
    update();

    const unsubscribe = onClientReady(() => {
      if (!snapshot.loading) {
        void loadAgentPresets();
      }
    });

    if (!hasLoaded && !snapshot.loading) {
      void loadAgentPresets();
    }

    return () => {
      listeners.delete(update);
      unsubscribe();
    };
  }, []);

  return state;
}

export function resetAgentPresetsForTest() {
  snapshot = {
    presets: [],
    error: null,
    loading: false,
  };
  hasLoaded = false;
  inflight = null;
  listeners.clear();
}
