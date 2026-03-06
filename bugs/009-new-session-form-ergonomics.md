# 009: New session form has several workflow and ergonomics gaps

Status: open
Owner: frontend

## Symptom

The new-session form is missing several expected workflow affordances:
- if the session list is filtered by project, opening `New Session` does not preselect that project
- the base branch control should be a searchable dropdown
- the current branch chooser is rendered as two separate controls: a text input and a dropdown/trigger, which is confusing
- `Cmd+Return` does not submit the form
- the create button does not show the keyboard shortcut inline

## Expected Behavior

- project-filter context carries into the dialog as the default selected project
- base branch is selected from one coherent searchable dropdown / combo box, not a duplicated input plus dropdown
- `Cmd+Return` creates the session
- the create button shows the shortcut inline with icons

## Evidence

User report from live use of the session-list and new-session dialog flow, including a screenshot showing both a freeform branch text field and a separate branch dropdown in the same form.

## Suspected Root Cause

The dialog currently behaves more like a generic form than a workflow-optimized creation flow tied to the session-list context.

## Spec Impact

Session creation UX and keyboard ergonomics.

## Next Action

- thread project-filter state into the dialog default selection
- collapse the branch chooser into one coherent searchable dropdown UX
- add `Cmd+Return` submission support and visible shortcut affordance on the submit button
