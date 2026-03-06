# 024: Session-list empty state duplicates the primary action

Status: open
Owner: frontend

## Symptom

When the session list is empty, Ship shows `New Session` in the toolbar and again inside the centered empty-state card.

## Expected behavior

There should be one clear primary CTA. The empty state should support the page, not duplicate the main action and compete with it.

## Evidence

- Screenshot from March 6, 2026 shows a top-right `New Session` button and a second `New Session` button inside the empty-state card.

## Suspected root cause

The empty-state design still assumes it owns the primary action even after the toolbar was redesigned around a single top-row CTA.

## Spec impact

- `r[ui.session-list.empty]`
- `r[ui.session-list.create]`

## Next action

- Remove the duplicate CTA from the empty-state card or demote it to supporting copy.
- Keep the toolbar `New Session` action as the page-level primary action.
