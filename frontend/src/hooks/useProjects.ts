import { useEffect, useState } from "react";
import { shipClient } from "../api/client";
import type { ProjectInfo } from "../generated/ship";

// r[proto.list-projects]
export function useProjects(): ProjectInfo[] {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);

  useEffect(() => {
    let active = true;

    async function fetchProjects() {
      const client = await shipClient;
      const list = await client.listProjects();
      if (active) setProjects(list);
    }

    fetchProjects();

    window.addEventListener("focus", fetchProjects);
    return () => {
      active = false;
      window.removeEventListener("focus", fetchProjects);
    };
  }, []);

  return projects;
}
