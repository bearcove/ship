import { useState } from "react";
import type { AgentKind } from "../generated/ship";

const CAPTAIN_KEY = "ship.captainKind";
const MATE_KEY = "ship.mateKind";

function readKind(key: string, fallback: AgentKind["tag"]): AgentKind {
  try {
    const stored = localStorage.getItem(key);
    if (stored === "Claude" || stored === "Codex") return { tag: stored };
  } catch {
    // ignore
  }
  return { tag: fallback };
}

function writeKind(key: string, kind: AgentKind) {
  try {
    localStorage.setItem(key, kind.tag);
  } catch {
    // ignore
  }
}

export function useAgentKindPrefs() {
  const [captainKind, setCaptainKindState] = useState<AgentKind>(() =>
    readKind(CAPTAIN_KEY, "Claude"),
  );
  const [mateKind, setMateKindState] = useState<AgentKind>(() => readKind(MATE_KEY, "Claude"));

  function setCaptainKind(kind: AgentKind) {
    setCaptainKindState(kind);
    writeKind(CAPTAIN_KEY, kind);
  }

  function setMateKind(kind: AgentKind) {
    setMateKindState(kind);
    writeKind(MATE_KEY, kind);
  }

  return { captainKind, setCaptainKind, mateKind, setMateKind };
}
