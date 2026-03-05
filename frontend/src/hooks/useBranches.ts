import { MOCK_BRANCHES } from "../mocks/data";

export function useBranches(projectName: string): string[] {
  return MOCK_BRANCHES[projectName] ?? ["main"];
}
