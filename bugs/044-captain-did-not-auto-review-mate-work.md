# 044: Captain did not review mate's work when mate finished

Status: open
Owner: backend

## Symptom

~7:50 — When the mate finished and signaled end-of-turn, the task moved to "ReviewPending" and a human-facing "Accept mate work" button appeared. The captain was idle and did nothing. The user had to manually click accept.

## Expected behavior

When the mate finishes (StopReason::EndTurn → ReviewPending), the backend should prompt the captain with a summary of what the mate did and ask it to review. The captain then calls `captain_accept` or `captain_steer` based on its assessment.

The human should not need to manually accept in the normal flow. The human-facing accept/steer buttons are for override, not the primary path.

## Root cause

The `handle_mate_stop_reason` handler transitions to ReviewPending but does not prompt the captain. The captain is left idle.

## Next action

- In `handle_mate_stop_reason` for EndTurn: inject a review prompt into the captain with a summary of mate output
- Captain then calls captain_accept or captain_steer
- Human accept/cancel buttons remain for override
- Update spec `r[captain.review.auto]`
