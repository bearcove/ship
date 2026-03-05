import { MOCK_PROJECTS } from "../mocks/data";
import type { ProjectInfo } from "../generated/ship";

// r[proto.list-projects]
export function useProjects(): ProjectInfo[] {
  return MOCK_PROJECTS;
}
