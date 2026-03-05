import { MOCK_SESSION_DETAIL } from "../mocks/data";
import type { SessionDetail } from "../types";

export function useSession(_id: string): SessionDetail {
  return MOCK_SESSION_DETAIL;
}
