import { useEffect, useState } from "react";
import { shipClient } from "../api/client";

// r[proto.list-branches]
export function useBranches(projectName: string): string[] {
  const [branches, setBranches] = useState<string[]>([]);

  useEffect(() => {
    if (!projectName) return;
    let active = true;

    async function fetchBranches() {
      const client = await shipClient;
      const list = await client.listBranches(projectName);
      if (active) setBranches(list);
    }

    fetchBranches();
    return () => {
      active = false;
    };
  }, [projectName]);

  return branches;
}
