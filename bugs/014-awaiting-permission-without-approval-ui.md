# 014: Agent can show AwaitingPermission with no visible approval affordance

Status: open
Owner: fullstack

## Symptom

The agent state can show `Awaiting Permission`, but the user is not given any visible UI to approve or deny the request.

## Expected Behavior

If an agent is awaiting permission, the corresponding permission request block or approval UI must be visible and actionable.

## Evidence

User screenshot showing:
- captain state badge: `Awaiting Permission`
- no obvious permission prompt or approval control in the visible feed

## Suspected Root Cause

Possible mismatch between:
- agent state transitions
- permission block creation/replay
- permission block rendering/filtering

The state appears to update, but the actionable permission surface does not.

## Spec Impact

Permission workflow visibility and usability.

## Next Action

- verify permission blocks are emitted, replayed, and rendered when agent state enters `AwaitingPermission`
- verify they are not being filtered out or lost during hydration/replay
- add a regression test for state + permission block consistency
