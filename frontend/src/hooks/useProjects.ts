import { MOCK_PROJECTS } from "../mocks/data";
import type { Project } from "../types";

// r[proto.list-projects]
export function useProjects(): Project[] {
  return MOCK_PROJECTS;
}
