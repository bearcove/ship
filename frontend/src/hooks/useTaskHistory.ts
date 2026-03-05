import { useEffect, useState } from "react";
import { shipClient } from "../api/client";
import type { TaskRecord } from "../generated/ship";

// r[session.single-task]
export function useTaskHistory(id: string): TaskRecord[] {
  const [history, setHistory] = useState<TaskRecord[]>([]);

  useEffect(() => {
    let active = true;

    shipClient
      .then((client) => client.getSession(id))
      .then((detail) => {
        if (active) setHistory(detail.task_history);
      });

    return () => {
      active = false;
    };
  }, [id]);

  return history;
}
