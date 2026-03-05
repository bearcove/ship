import { createContext, useContext, useState } from "react";
import type { SessionScenarioKey, SessionListScenarioKey } from "../types";

interface ScenarioContextValue {
  sessionScenario: SessionScenarioKey;
  setSessionScenario: (k: SessionScenarioKey) => void;
  sessionListScenario: SessionListScenarioKey;
  setSessionListScenario: (k: SessionListScenarioKey) => void;
}

const ScenarioContext = createContext<ScenarioContextValue>(null!);

export function ScenarioProvider({ children }: { children: React.ReactNode }) {
  const [sessionScenario, setSessionScenario] = useState<SessionScenarioKey>("happy-path");
  const [sessionListScenario, setSessionListScenario] = useState<SessionListScenarioKey>("normal");
  return (
    <ScenarioContext.Provider
      value={{ sessionScenario, setSessionScenario, sessionListScenario, setSessionListScenario }}
    >
      {children}
    </ScenarioContext.Provider>
  );
}

export function useScenario() {
  return useContext(ScenarioContext);
}
