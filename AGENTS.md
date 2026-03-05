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

You work in a git worktree on your own branch. Commit your work when done. The coordinator handles merging and rebasing — you don't need to worry about that.

Before starting work, make sure your branch is up to date with main (`git rebase main` if needed).

Stay within the scope of the assigned task. If a frontend task depends on a backend RPC, shared type, generated artifact, or other backend-owned surface that is not already present in your worktree, stop and report the missing dependency to the coordinator instead of implementing backend work from the frontend worktree. The same rule applies in reverse for backend tasks that depend on frontend work.

## Coordinator git safety

- Coordinator git operations that mutate repository state MUST be run sequentially, never in parallel
- This includes `git add`, `git commit`, `git merge`, `git rebase`, `git push`, and any command that can create `.git/index.lock`
- Parallel tool use is only allowed for read-only git inspection such as `git status`, `git log`, `git diff`, and `git show`
