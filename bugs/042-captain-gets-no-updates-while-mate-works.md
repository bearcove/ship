# 042: Captain receives no updates while mate is working

Status: open
Owner: backend

## Symptom

~6:03 — The captain's panel showed "Delegating to the mate." for the entire ~5 minutes the mate was working. The captain had no visibility into what the mate was doing, what had been completed, or whether anything went wrong.

## Expected behavior

The captain should receive periodic updates about mate progress — at minimum when plan steps complete, when the mate hits a blocker, or on a time interval. This gives the captain enough signal to:
- Decide whether to steer
- Know what context to pass when assigning the next task
- Detect if the mate is going in the wrong direction

## Related

- Bug 036 (captain lacks mate context on next assign)

## Next action

- Implement periodic mate status injection into captain context
- At minimum: inject a summary when a plan step completes
- Include: completed steps, current step, any blockers
- See bug 036 for the assign-time context injection
