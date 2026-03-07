# 032: Multi-session UI is zoomed into one session at a time

Status: open
Owner: frontend

## Symptom

The session list is just a navigation screen. When running multiple sessions in parallel, there is no way to see what each one is doing at a glance — you have to navigate in and out per session.

## Expected behavior

Vertical tabs along the side let you switch between active sessions without leaving the current context. Each tab shows the session's current status (agent state, task description) so you can triage at a glance.

## Next action

- Design vertical tab layout for the session view
- Each tab: project name, task description (truncated), status badge
- Active session fills the main pane
