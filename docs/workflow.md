# Ship Development Workflow

## Roles

- **Coordinator**: the Claude session on `main`. Merges work, runs tracey annotation passes, writes agent prompts, dispatches agents.
- **Frontend agent**: works in `~/bearcove/ship-frontend` on the `frontend` branch.
- **Backend agent**: works in `~/bearcove/ship-backend` on the `backend` branch.

## Worktree layout

| Path | Branch | Purpose |
|---|---|---|
| `~/bearcove/ship` | `main` | Integration point. Coordinator works here. |
| `~/bearcove/ship-frontend` | `frontend` | Frontend agent works here. |
| `~/bearcove/ship-backend` | `backend` | Backend agent works here. |

Worktrees are **persistent** — don't remove them between passes.

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
