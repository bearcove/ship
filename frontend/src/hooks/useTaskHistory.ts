import { useScenario } from "../context/ScenarioContext";
import { SESSION_SCENARIOS } from "../mocks/data";
import type { Task } from "../types";

// r[session.single-task]
export function useTaskHistory(_id: string): Task[] {
  const { sessionScenario } = useScenario();
  return SESSION_SCENARIOS[sessionScenario].taskHistory;
}
