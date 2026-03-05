import { MOCK_CAPTAIN_EVENTS, MOCK_MATE_EVENTS } from "../mocks/data";
import type { ContentBlock, Role } from "../types";

export function useSessionEvents(_id: string, role: Role): ContentBlock[] {
  return role === "captain" ? MOCK_CAPTAIN_EVENTS : MOCK_MATE_EVENTS;
}
