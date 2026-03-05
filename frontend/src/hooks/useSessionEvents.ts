import { useScenario } from "../context/ScenarioContext";
import { SESSION_SCENARIOS } from "../mocks/data";
import type { ContentBlock, Role } from "../types";

// r[event.subscribe]
export function useSessionEvents(_id: string, role: Role): ContentBlock[] {
  const { sessionScenario } = useScenario();
  const scenario = SESSION_SCENARIOS[sessionScenario];
  return role === "captain" ? scenario.captainEvents : scenario.mateEvents;
}
