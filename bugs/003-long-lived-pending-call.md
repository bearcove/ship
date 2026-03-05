# 003: Session/task appears to keep a long-lived pending RPC call

Status: open
Owner: backend

## Symptom

During a session or task, there appears to be a request/call that stays pending for the entire duration.

## Expected Behavior

`create_session` should return promptly. Ongoing updates should arrive over the event subscription stream rather than through a long-lived request that stays pending.

## Evidence

Coordinator report from live use: the system behaves as though there is a pending call for the whole session/task.

## Suspected Root Cause

One of the prompt/task orchestration paths may be incorrectly tied to request lifetime instead of fully decoupling mutation requests from streaming updates.

## Spec Impact

Conflicts with the intended request/response plus event-stream model.

## Next Action

- Trace request lifetimes for `create_session`, `assign`, `steer`, and related flows.
- Verify that only `subscribe_events` is long-lived.
- Inspect browser network activity and server task spawning behavior to find the stuck call path.
