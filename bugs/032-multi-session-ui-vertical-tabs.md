# 032: Multi-session UI — vertical tabs replace session list navigation

Status: open
Owner: frontend

## Symptom

The session list is a separate navigation screen. When running multiple sessions in parallel there's no way to see what each is doing at a glance. The session view is full-screen with no context about other sessions.

## Expected behavior

Vertical tabs on the left side of the screen, always visible:
- Each tab represents one session
- Tab shows: project name + current task description (truncated) + agent state indicator
- Clicking a tab switches the main pane to that session
- No separate "session list page" needed for day-to-day use
- Current task description in the tab replaces the task bar at the bottom (which is being removed per bug 039)

The session list page can still exist as a management view (create session, see all sessions) but it's not the primary navigation.

## Next action

- Design and implement vertical tab strip component
- Each tab: project name, task description (from current_task, truncated), status/state badge
- Remove task bar from session view (task context lives in the tab)
- Hook up tab switching to session routing
