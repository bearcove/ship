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
