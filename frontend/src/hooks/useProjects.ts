import { useEffect, useState } from "react";
import { getShipClient, invalidateShipClient, onClientReady } from "../api/client";
import type { ProjectInfo } from "../generated/ship";

// r[proto.list-projects]
export function useProjects(): ProjectInfo[] {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);

  useEffect(() => {
    let active = true;

    async function fetchProjects() {
      try {
        const client = await getShipClient();
        const list = await client.listProjects();
        if (active) setProjects(list);
      } catch (e) {
        invalidateShipClient(`listProjects failed: ${e}`);
      }
    }

    fetchProjects();

    const unsubReady = onClientReady(() => void fetchProjects());
    window.addEventListener("focus", fetchProjects);
    return () => {
      active = false;
      unsubReady();
      window.removeEventListener("focus", fetchProjects);
    };
  }, []);

  return projects;
}
