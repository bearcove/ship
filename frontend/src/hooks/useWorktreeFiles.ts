import { useEffect, useState } from "react";
import { getShipClient } from "../api/client";

// r[ui.composer.file-mention]
export function useWorktreeFiles(sessionId: string): string[] {
  const [files, setFiles] = useState<string[]>([]);

  useEffect(() => {
    let active = true;

    async function fetchFiles() {
      const client = await getShipClient();
      const list = await client.listWorktreeFiles(sessionId);
      if (active) setFiles(list);
    }

    fetchFiles();

    window.addEventListener("focus", fetchFiles);
    return () => {
      active = false;
      window.removeEventListener("focus", fetchFiles);
    };
  }, [sessionId]);

  return files;
}
