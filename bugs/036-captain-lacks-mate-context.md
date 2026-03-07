# 036: Captain does not receive mate context when assigning next task

Status: open
Owner: backend

## Symptom

When the captain assigns a new task (or steers), it has no structured view of what the mate has been doing. It cannot make an informed decision about whether to use `keep=true` or restart the mate with fresh context.

## Expected behavior

When prompting the captain, the backend injects a summary of the mate's recent activity: git status, last few mate outputs/thoughts, current task status. This gives the captain enough signal to decide whether to keep the mate's context or restart it.

## Ideas

- Include `git status` output in captain context injection
- Include last N mate messages/thoughts (truncated)
- Proactive updates: periodically send the captain "here's what the mate is currently doing" while the mate is working, so the captain stays informed without waiting

## Next action

- Define the context injection format
- Add mate status summary to captain prompt in `dispatch_steer_to_mate` / `captain_tool_assign`
- Spec rule: `captain.context.mate-status`
