# 006: Session list toolbar hierarchy is wrong

Status: open
Owner: frontend

## Symptom

The session list top bar uses the wrong action hierarchy:
- `+ Add Project` appears as a large separate button
- `New Session` is also a large CTA
- `All projects` is not in the top-left slot

This makes the top bar feel heavier than it should and gives secondary actions too much weight.

## Expected Behavior

- `New Session` should be the main inline action in the top row of the session list
- `All projects` should sit at the top-left as the primary filter control
- `Add Project` should not be a separate large button; it should live as an option at the bottom of the project dropdown

## Evidence

User report and screenshot of the current session-list toolbar showing both `Add Project` and `New Session` as filled buttons.

## Suspected Root Cause

The current layout treats project management and session creation as sibling CTAs instead of giving session creation a clear primary action and folding project creation into the project selector flow.

## Spec Impact

Primarily UX/layout polish around the session list and project filter entry points.

## Next Action

- Redesign the session-list top row around one primary CTA and one filter control
- Move `Add Project` into the project dropdown as a terminal action
- Verify spacing and visual hierarchy once the separate button is removed
