# 039: Remove explicit task creation — human just talks to the captain

Status: closed
Owner: backend + frontend

## Symptom

~2:11 — The "New Task" button creates a session-level task record, which blocks `captain_assign` when the captain tries to delegate to the mate. The captain had to cancel the human-created task before it could proceed.

## Root cause / Design issue

There should be no explicit task creation UI. The human talks to the captain in natural language. The captain decides when to call `captain_assign`, which is the only thing that creates a task record.

## Expected behavior

- Remove the "New Task" button and its dialog
- Remove the task bar at the bottom entirely (see bug 032 — current task lives in the vertical tab)
- Remove (or repurpose) the `proto.assign` RPC
- Human input is just a freeform message to the captain via the composer
- `captain_assign` is the sole task-creation path
- Update spec `r[proto.assign]` and `r[task.assign]` accordingly

## Next action

- Delete TaskBar component (or strip it to just read-only task status if needed elsewhere)
- Remove "New Task" dialog
- Remove `assign` from the Ship RPC service
- The captain composer input is the primary human→captain channel from session start
