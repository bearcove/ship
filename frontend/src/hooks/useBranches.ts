import { MOCK_BRANCHES } from "../mocks/data";

// r[proto.list-branches]
export function useBranches(projectName: string): string[] {
  return MOCK_BRANCHES[projectName] ?? ["main"];
}
