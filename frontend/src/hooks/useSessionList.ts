import { MOCK_SESSIONS } from "../mocks/data";
import type { SessionSummary } from "../types";

export function useSessionList(projectFilter?: string): SessionSummary[] {
  if (!projectFilter) return MOCK_SESSIONS;
  return MOCK_SESSIONS.filter((s) => s.projectName === projectFilter);
}
