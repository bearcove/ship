# Stop action cancels the task instead of just stopping agents

Hitting "stop" (the stop button in the UI, or the stop action) cancels the active task entirely. This means the task record is cleared and cannot be resumed or redirected.

The desired behaviour is: stop should halt the captain and mate agents but leave the task intact, so the user can resume or redirect later. The task should remain in its current state (with any progress made so far preserved), and the user should be able to re-engage the agents on the same task without having to re-assign it from scratch.
