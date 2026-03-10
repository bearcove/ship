# 023 — Mate can request a worktree reset

## Problem

Sometimes the mate realizes partway through a task that they've gone down the
wrong path and need to start fresh. There's currently no way to do this cleanly.
Allowing the mate to just `git reset --hard` themselves would be destructive and
uncontrolled.

## Desired behavior

The mate should be able to call a tool (e.g. `request_reset`) that:

1. Blocks the mate, interrupting the captain with a message explaining why they
   want to reset.
2. The captain reviews and either approves or rejects.
3. On **approval**: a new branch is checked out from the same base as the
   original worktree branch (not from the current HEAD). The old branch is
   preserved (not deleted) so the work isn't thrown away — it just becomes
   unreachable from the mate's active branch. The mate unblocks and starts
   clean.
4. On **rejection**: the mate unblocks with the captain's feedback and continues
   on their current branch.

## Implementation notes

- The new branch name should follow the same `ship/{session_id_short}/{slug}`
  convention, perhaps with a `-reset-N` suffix.
- The base ref to branch from is already stored as the worktree's original base
  branch — reuse that.
- Do NOT delete the old branch. Keeping it means the work can be inspected or
  cherry-picked later.
- This is similar to the mid-task `set_plan` blocking pattern: block mate →
  interrupt captain → `captain_accept` approves / `captain_steer` rejects.
  Reuse `plan_change_reply` or add a `reset_reply` field to `PendingMcpOps`.
