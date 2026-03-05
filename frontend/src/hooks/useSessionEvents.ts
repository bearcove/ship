import { useScenario } from "../context/ScenarioContext";
import { SESSION_SCENARIOS } from "../mocks/data";
import type { ContentBlock, Role } from "../generated/ship";

// r[event.subscribe]
export function useSessionEvents(_id: string, role: Role): ContentBlock[] {
  const { sessionScenario } = useScenario();
  const scenario = SESSION_SCENARIOS[sessionScenario];
  return role.tag === "Captain" ? scenario.captainEvents : scenario.mateEvents;
}
