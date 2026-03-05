import { MOCK_TASK_HISTORY } from "../mocks/data";
import type { Task } from "../types";

export function useTaskHistory(_id: string): Task[] {
  return MOCK_TASK_HISTORY;
}
