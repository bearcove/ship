# Mate plan/tool gating is broken

Various MCP tools are gated on the mate first doing `plan_create`. What actually happens: mate does all the work, tries `mate_submit`, it fails because the worktree rule requires `plan_create` first. Then the mate retroactively creates a plan, marks all steps complete, and resubmits — pure theater.

Observed sequence:
1. Mate does all the actual work
2. `ship/mate_submit` → **Failed** ("blocked by the worktree rule requiring `plan_create` first")
3. Mate calls `ship/plan_create` retroactively
4. Mate calls `ship/plan_step_complete` for each step
5. Mate resubmits

Two possible fixes:
1. Let the mate just work without plan gating
2. Force plan creation earlier through better prompting + actually gate tools properly

Part of the problem: the mate may be using builtin Todo tools instead of our Plan tools, bypassing the gate entirely.
