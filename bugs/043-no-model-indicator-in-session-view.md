# 043: No exact model indicator in session view

Status: closed
Owner: backend + frontend

## Symptom

~1:28 — The user couldn't tell which specific model the captain was running (Opus? Sonnet?). The session view shows Claude/Codex via icon already. What's missing is the exact model string — e.g. `claude-opus-4-6` vs `claude-sonnet-4-6`.

## What was tried and rejected

Adding a `Badge` showing "Claude" or "Codex" — this duplicates information already conveyed by the icon and adds nothing.

## Root cause

`AgentSnapshot` only carries `AgentKind` (Claude vs Codex). The actual model string is not surfaced anywhere in the type system.

## Expected behavior

The agent panel header shows the exact model string, e.g. `opus-4-6` or `sonnet-4-6`, as a small subdued label under or next to the role name.

## Next action

1. Backend: add `model: Option<String>` (or similar) to `AgentSnapshot` in `ship-types`
2. Backend: populate it from the agent config when the session is created
3. Codegen: regenerate TypeScript types
4. Frontend: display the model string in the agent panel header if present
