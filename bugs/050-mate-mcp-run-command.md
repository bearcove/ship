# 050: Ship-controlled run_command MCP tool for the mate

Owner: backend

## What

Add a `run_command` MCP tool. Takes `command` and optional `cwd` (relative to worktree). Runs the command in the worktree. No permission prompts — Ship controls the boundary.

Integrates with the existing dangerous command blocking (guardrails from bug 042): `git checkout/restore/clean/reset` and broad `rm -rf` patterns are rejected before execution.

## Why

Replaces raw ACP shell execution capability. Ship-controlled means no per-action permission prompts for safe commands, and dangerous commands are blocked at the source.

## Next action

- Add `run_command` tool to mate MCP server
- Execute in worktree, capture stdout/stderr
- Reject dangerous commands (reuse existing `is_dangerous_command`)
- Reasonable timeout and output size limits
