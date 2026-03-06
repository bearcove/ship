# 010: Session view chrome and status presentation are awkward

Status: open
Owner: frontend

## Symptom

Several session-view UI elements are making the interface harder to scan:
- `Autonomous [toggle] Human-in-the-loop` causes layout jitter; the right-side label shifts the row when toggled
- `Working` / `Idle` status appears in the top header area instead of inline near the feed/input area
- agent kinds are repeatedly rendered as text labels (`Claude` / `Codex`) instead of compact icons

## Expected Behavior

- autonomy toggle should remain visually stable without the shifting text labels
- agent status should appear inline at the bottom near the feed/input controls
- agent identity should use icons rather than repeating textual labels everywhere

## Evidence

User report from live use of the session view.

## Suspected Root Cause

The current session-view chrome overemphasizes static labels in the header and underuses compact visual affordances in the activity area.

## Spec Impact

Session-view usability and presentation polish.

## Next Action

- remove the autonomy mode text labels around the toggle
- move transient agent status to the bottom interaction area
- introduce agent-kind icons and replace repeated textual labels where appropriate
