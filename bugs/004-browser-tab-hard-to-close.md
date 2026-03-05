# 004: Browser tab becomes hard to close under failure conditions

Status: open
Owner: shared

## Symptom

When the browser gets into a bad state, the Chrome tab becomes difficult to close and may require a second close attempt, suggesting a runaway loop or heavily blocked runtime.

## Expected Behavior

Even when connections fail or the app enters an error state, the tab should remain responsive and easy to close.

## Evidence

Coordinator report from live use: when things go wrong, the tab becomes hard to close.

## Suspected Root Cause

Possible reconnect storm, infinite loop, event flood, or runtime issue in Ship or Roam. If it is in Roam, fix it there rather than adding a workaround in Ship.

## Spec Impact

This is primarily a stability/runtime bug, but it may also interact with connection-loss handling.

## Next Action

- Reproduce with browser devtools and performance tools.
- Check for repeated reconnect attempts, repeated state updates, or runaway subscriptions.
- Determine whether the bug originates in Ship app code or Roam runtime code.
