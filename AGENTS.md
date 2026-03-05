# Ship — Agent Instructions

## TypeScript

- Use `tsgo` (from `@typescript/native-preview`), not `tsc`, for all typechecking
- Run: `pnpm exec tsgo --noEmit`
- The `build` and `typecheck` scripts in `frontend/package.json` already use `tsgo`

## Linting and formatting

- `oxlint` for linting TypeScript/JavaScript
- `oxfmt` for formatting TypeScript/JavaScript
- Both run automatically via lefthook pre-commit hooks
- Run manually: `pnpm exec oxlint frontend/src/` or `pnpm exec oxfmt --write frontend/src/`

## Package manager

- pnpm, not npm or yarn
- Workspace root is at repo root, `frontend/` is the only workspace package

## Pre-commit hooks

Managed by lefthook (`lefthook.yml`). Runs in parallel:
1. `oxfmt` — formats staged .ts/.tsx files
2. `oxlint` — lints staged .ts/.tsx files
3. `tsgo --noEmit` — typechecks the frontend

## Radix Themes docs

Fetch component docs as markdown:

```
https://www.radix-ui.com/themes/docs/components/{component-name}.md
```

Examples:
- `https://www.radix-ui.com/themes/docs/components/card.md`
- `https://www.radix-ui.com/themes/docs/components/segmented-control.md`
- `https://www.radix-ui.com/themes/docs/components/select.md`

Use this before assuming what props a component accepts.

## Spec coverage (Tracey)

Ship is spec-first. Every requirement in `docs/spec/ship.md` has an ID like `r[session.create]`. When you implement or test a requirement, annotate your code with Tracey markers.

### In Rust

```rust
// r[session.create]
pub async fn create_session(&self, req: CreateSessionRequest) -> Result<SessionId, Error> {
```

### In TypeScript

```typescript
// r[ui.session-list.layout]
export function SessionListPage() {
```

### Rules

- Place the `// r[requirement.id]` comment directly above the function, struct, component, or block that implements it
- One annotation per requirement. If a requirement is implemented across multiple places, annotate the primary location.
- For test functions, use `// r[requirement.id]` above the test to mark verification
- Check `docs/spec/ship.md` for the exact requirement IDs — don't guess
- Run `tracey status` (if available) to check coverage

## Development workflow

Ship uses git worktrees so multiple agents can work in parallel without conflicts.

### Worktree layout

| Path | Branch | Purpose |
|---|---|---|
| `~/bearcove/ship` | `main` | Integration point. No direct edits here. |
| `~/bearcove/ship-frontend` | `frontend` | Frontend agent works here. |
| `~/bearcove/ship-backend` | `backend` | Backend agent works here. |

### Merge cycle

1. Agents commit to their branches in their worktrees
2. When an agent's work is ready, rebase its branch onto main and fast-forward merge:
   ```
   cd ~/bearcove/ship-frontend && git rebase main
   cd ~/bearcove/ship && git merge --ff-only frontend
   ```
3. After merging, push main and rebase the other worktree:
   ```
   cd ~/bearcove/ship && git push
   cd ~/bearcove/ship-backend && git rebase main
   ```
4. Between agent passes (after merge, before next prompt): add Tracey annotations to code that's missing them, check `tracey status`, commit on main
5. Agents pull main into their branches before starting new work

### Rules

- Never edit source files directly on main while agents are working — it causes merge conflicts
- AGENTS.md and docs/ are safe to edit on main (agents don't modify them)
- Tracey annotation passes happen between merges, not during agent work
- Each agent's worktree is persistent — don't remove it between passes
