import { useScenario } from "../context/ScenarioContext";
import { SESSION_SCENARIOS } from "../mocks/data";
import type { SessionDetail } from "../types";

// r[proto.get-session]
export function useSession(_id: string): SessionDetail {
  const { sessionScenario } = useScenario();
  return SESSION_SCENARIOS[sessionScenario].detail;
}
