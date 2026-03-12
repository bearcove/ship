# Accept clears active task on failure, preventing retry

When `captain_accept` fails (e.g. due to a merge conflict during the merge step), it clears the active task from the session. This means the captain cannot retry the accept once the conflict is resolved — the task is gone.

The accept operation should leave the task intact on failure so the captain can attempt to accept again after the underlying issue (merge conflict, etc.) is resolved.

Fix: ensure that `captain_accept` only clears the active task on success. On failure, the task and its associated state should remain on the session so the operation can be retried.
