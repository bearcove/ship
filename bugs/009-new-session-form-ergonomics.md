# 009: New session form has several workflow and ergonomics gaps

Status: open
Owner: frontend

## Symptom

The new-session form is missing several expected workflow affordances:
- if the session list is filtered by project, opening `New Session` does not preselect that project
- the base branch control should be a searchable dropdown
- `Cmd+Return` does not submit the form
- the create button does not show the keyboard shortcut inline

## Expected Behavior

- project-filter context carries into the dialog as the default selected project
- base branch is selected from a searchable dropdown / combo box
- `Cmd+Return` creates the session
- the create button shows the shortcut inline with icons

## Evidence

User report from live use of the session-list and new-session dialog flow.

## Suspected Root Cause

The dialog currently behaves more like a generic form than a workflow-optimized creation flow tied to the session-list context.

## Spec Impact

Session creation UX and keyboard ergonomics.

## Next Action

- thread project-filter state into the dialog default selection
- replace the branch field with a searchable dropdown UX
- add `Cmd+Return` submission support and visible shortcut affordance on the submit button
