# 025: Session-list project context does not feel persistent

Status: open
Owner: frontend

## Symptom

The user returns to the session list and sees `All projects` selected even though they were just working in `helloworld`. Opening `New Session` does correctly preselect `helloworld`, but the list/filter state itself does not feel stable.

## Expected behavior

The current project context should feel consistent across session-list and new-session flows. If the user was working inside one project, the session list should not snap back to an unrelated default without a clear reason.

## Evidence

- User report from March 6, 2026: the list showed `All projects`, but the new-session dialog still preselected `helloworld`.
- The centered session-list panel also changes size when switching between global and project-scoped empty states.

## Suspected root cause

Project filter state and new-session defaulting are derived from different sources, so the dialog remembers project context better than the list UI does.

## Spec impact

- `r[ui.session-list.project-filter]`
- `r[ui.session-list.empty]`

## Next action

- Decide the canonical source of current project context for the session list.
- Keep empty-state layout stable when switching between all-projects and project-scoped views.
