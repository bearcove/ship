# Ship Development Workflow

## Roles

- **Coordinator**: the Claude session on `main`. Merges work, runs tracey annotation passes, writes agent prompts, dispatches agents.
- **Frontend agent**: works in `~/bearcove/ship-frontend` on the `frontend` branch.
- **Backend agent**: works in `~/bearcove/ship-backend` on the `backend` branch.
- **Fullstack agent**: works in `~/bearcove/ship-fullstack` on the `fullstack` branch when one agent owns both backend and frontend work for a pass.

## Worktree layout

| Path | Branch | Purpose |
|---|---|---|
| `~/bearcove/ship` | `main` | Integration point. Coordinator works here. |
| `~/bearcove/ship-frontend` | `frontend` | Frontend agent works here. |
| `~/bearcove/ship-backend` | `backend` | Backend agent works here. |
| `~/bearcove/ship-fullstack` | `fullstack` | Single agent owns both backend and frontend work for one pass. |

Worktrees are **persistent** — don't remove them between passes.

## Dispatch modes

- Use the split `frontend` / `backend` lanes when the work can be cleanly divided and sequenced.
- Use the `fullstack` lane when one pass must change backend and frontend together.
- Do not dispatch the `fullstack` lane in parallel with either split lane on overlapping source files.
- Before dispatching any mode, the coordinator must prepare the target worktree so its branch already matches current `main`.

## Merge cycle

1. Agent commits to its branch in its worktree.
2. Coordinator rebases the agent branch onto main and fast-forward merges:
   ```
   cd ~/bearcove/ship-frontend && git rebase main
   cd ~/bearcove/ship && git merge --ff-only frontend
   ```
3. Push main, rebase the other worktree:
   ```
   cd ~/bearcove/ship && git push
   cd ~/bearcove/ship-backend && git rebase main
   ```
4. **Between merges**: coordinator does a tracey annotation pass on main, commits, pushes.
5. Before dispatching any new prompt, coordinator rebases both agent worktrees onto the current `main` so they contain the latest docs, bug files, generated types, and coordinator-side changes.
6. Agents then start new work from those rebased worktrees.

## Task dependency ordering

- The coordinator must model dependencies between tasks explicitly before dispatching prompts.
- If a frontend task depends on a backend RPC, shared type, generated artifact, or any other backend-owned surface, the backend task must land on `main` first.
- Only after that backend dependency is merged, pushed, and the agent worktrees are refreshed from `main` may the frontend task be dispatched.
- In that situation, the frontend prompt must treat the backend surface as an existing prerequisite, not as work to be invented during the frontend pass.
- If the required backend surface is missing when the frontend agent starts, that is a blocker to report back to the coordinator, not an invitation to edit backend code from the frontend worktree.
- In a `fullstack` pass, the same dependency rule still applies, but inside one prompt:
  1. backend/shared/spec work first
  2. codegen/regenerated artifacts next
  3. frontend consumer work last
- A `fullstack` prompt must make that order explicit instead of mixing backend and frontend steps together loosely.

## Prompt scoping

- Prompts must define clear ownership boundaries.
- Frontend prompts should be scoped to `frontend/src/**` unless the task explicitly says otherwise.
- Backend prompts should be scoped to `crates/**`, shared protocol crates, codegen, and other backend-owned files unless the task explicitly requires a paired frontend follow-up.
- Do not give the frontend agent backend requirement IDs as implementation targets unless the task is intentionally cross-stack and sequenced that way.
- When a task depends on another task, the dependent task should be described as the final consumer step, not as a speculative implementation against a future API.

## Branch shape invariants

- Agent branches must stay clean fast-forward candidates for `main`.
- After coordinator merges agent work into `main`, the corresponding worktree branch should normally be either identical to `main` or a simple descendant of it.
- Rebasing a worktree branch onto `main` must not duplicate already-merged coordinator commits under new SHAs. If that happens, stop and repair the branch before dispatching more work.

## Recovery procedure

If a worktree branch stops being a clean fast-forward candidate for `main`:

1. Do not merge that branch directly.
2. Cherry-pick only the reviewed tip commits that have not already landed on `main`.
3. Push `main`.
4. Recreate a clean worktree branch from `main`:
   ```
   cd ~/bearcove/ship-frontend && git checkout -B frontend main
   cd ~/bearcove/ship-backend && git checkout -B backend main
   cd ~/bearcove/ship-fullstack && git checkout -B fullstack main
   ```
5. Only after the branch has been reset to match `main` should the next agent task begin.

## What lives where

- **AGENTS.md**: instructions for agents (tooling, conventions, tracey annotation syntax). Agents read this.
- **docs/workflow.md**: orchestration workflow for the coordinator. Agents don't need this.
- **docs/spec/ship.md**: the spec. Source of truth for all requirement IDs.

## Tracey annotation passes

Done by the coordinator on main, between agent merges. Steps:

1. Read the spec (`docs/spec/ship.md`) to get real rule IDs. **Never invent IDs.**
2. Add `// r[rule.id]` markers to code that implements requirements.
3. Add `// r[verify rule.id]` markers to tests that verify requirements.
4. Run `tracey query validate` to confirm zero unknown references.
5. Commit on main.

## Pre-commit hooks (lefthook)

- `capn` — rustfmt, clippy, cargo-shear, edition checks
- `oxfmt` — formats staged TS/TSX
- `oxlint` — lints staged TS/TSX
- `tsgo --noEmit` — typechecks frontend
- `tracey query validate` — catches unknown/invented requirement IDs

## Rules

- Never edit source files on main while agents are working — causes merge conflicts.
- AGENTS.md and docs/ are safe to edit on main.
- Tracey annotation passes happen between merges, not during agent work.
- Always read the spec before writing annotations. Don't guess rule IDs.
- If the coordinator adds or changes files that agent prompts depend on (for example bug tracker files, docs, or generated artifacts), the coordinator must rebase the agent worktrees before sending those prompts.
- Coordinator git mutations must be run sequentially, never in parallel.
- Never touch an active agent worktree while that agent is working. Rebase, reset, or branch repair only happens between tasks.
- When topology changes (for example switching from split lanes to a fullstack lane), update this file first so the workflow is explicit before dispatch.
