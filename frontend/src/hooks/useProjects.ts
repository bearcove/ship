import { MOCK_PROJECTS } from "../mocks/data";
import type { Project } from "../types";

export function useProjects(): Project[] {
  return MOCK_PROJECTS;
}
