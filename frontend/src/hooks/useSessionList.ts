import { useScenario } from "../context/ScenarioContext";
import { SESSION_LIST_SCENARIOS } from "../mocks/data";
import type { SessionSummary } from "../generated/ship";

// r[proto.list-sessions]
export function useSessionList(projectFilter?: string): SessionSummary[] {
  const { sessionListScenario } = useScenario();
  const sessions = SESSION_LIST_SCENARIOS[sessionListScenario];
  if (!projectFilter) return sessions;
  return sessions.filter((s) => s.project === projectFilter);
}
