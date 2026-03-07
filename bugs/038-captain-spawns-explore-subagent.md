# 038: Captain spawned an Explore subagent

Status: open
Owner: backend

## Symptom

~0:29 — The captain launched an Explore agent to research the codebase before deciding how to approach the task. This is not the right model: the captain should think with its own context, not spin up subagents.

## Expected behavior

The captain does not have access to ACP filesystem/explore capabilities. It reviews, delegates, and steers. If it needs to understand the codebase, that's what the mate is for.

## Root cause

The captain is configured with ACP capabilities that allow it to spawn subagents. These should be stripped. See also bug 037 (mate shouldn't have raw filesystem either).

## Next action

- Audit captain ACP config — remove explore/subagent capabilities
- Captain should only have: Ship MCP tools (captain_assign, captain_steer, captain_accept, captain_cancel, captain_notify_human)
