# 043: No model indicator in session view

Status: open
Owner: frontend

## Symptom

~1:28 — The user couldn't tell which model the captain was running on (Opus? Sonnet?). The session view shows "Captain" and "Mate" labels but nothing about which model/agent kind is configured.

## Expected behavior

The agent panel header should show the agent kind — at minimum Claude vs Codex, ideally also the specific model tier (Opus/Sonnet/Haiku). This matters for understanding cost and capability.

## Next action

- Add agent kind badge to captain/mate panel headers (already have AgentKind in SessionDetail)
- Consider showing model tier if available from ACP
