import { useEffect, useState } from "react";
import type { ProjectInfo } from "../generated/ship";
import { onProjectListChanged } from "./useGlobalEvents";

// r[proto.list-projects]
export function useProjects(): ProjectInfo[] {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);

  useEffect(() => {
    return onProjectListChanged(setProjects);
  }, []);

  return projects;
}
