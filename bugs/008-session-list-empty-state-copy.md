# 008: Session list empty-state copy is redundant

Status: open
Owner: frontend

## Symptom

When filtering the session list by project, the empty state can show redundant messaging such as:

```text
No sessions yet. No sessions in ship.
```

## Expected Behavior

The empty state should show one clear message, not stacked overlapping variants.

## Evidence

User report from the filtered session-list view.

## Suspected Root Cause

Multiple empty-state branches are rendering at the same time instead of choosing one message appropriate to the current filter context.

## Spec Impact

Session-list UX copy and conditional rendering behavior.

## Next Action

- Audit empty-state rendering for filtered vs unfiltered session lists
- Collapse the copy to one message chosen from the current state
