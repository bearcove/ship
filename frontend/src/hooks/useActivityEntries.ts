import { useEffect, useState } from "react";
import type { ActivityEntry } from "../generated/ship";
import { onActivityChanged } from "./useGlobalEvents";

export function useActivityEntries(): ActivityEntry[] {
  const [entries, setEntries] = useState<ActivityEntry[]>([]);
  useEffect(() => onActivityChanged(setEntries), []);
  return entries;
}
