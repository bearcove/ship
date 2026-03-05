import { useScenario } from "../context/ScenarioContext";
import { SESSION_SCENARIOS } from "../mocks/data";
import type { TaskRecord } from "../generated/ship";

// r[session.single-task]
export function useTaskHistory(_id: string): TaskRecord[] {
  const { sessionScenario } = useScenario();
  return SESSION_SCENARIOS[sessionScenario].taskHistory;
}
