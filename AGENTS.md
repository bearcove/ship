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

Managed by lefthook (`lefthook.yml`). Runs sequentially:
1. `cargo xtask codegen` — regenerates TypeScript bindings and stages them
2. `capn` — rust-side checks
3. `oxfmt` — formats staged .ts/.tsx files
4. `oxlint` — lints staged .ts/.tsx files
5. `tsgo --noEmit` — typechecks the frontend

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

## Generated code

- Never hand-edit generated files.
- Change the source-of-truth inputs only.
- Then run `cargo xtask codegen` and commit the regenerated artifacts.
- If generated output looks wrong, fix the schema/codegen/runtime input that produced it rather than patching the output directly.

## Architecture: key files and layers

### Agent MCP tools

The captain and mate each get their own MCP server with a distinct set of tools.

**Captain** (`crates/ship-server/src/captain_mcp_server.rs`):
- `captain_assign` — assign a task to the mate (title + description)
- `captain_steer` — send direction to the mate on the current task
- `captain_accept` — accept the mate's submitted work
- `captain_cancel` — cancel the current task
- `captain_notify_human` — ask the human for guidance (blocks until response)

The captain has NO file, terminal, or code access. It reviews and delegates.

**Mate** (`crates/ship-server/src/mate_mcp_server.rs`):
- `run_command` — shell command via `sh -c` in the worktree
- `read_file` — read a file with line numbers, optional offset/limit
- `write_file` — write a file (Rust files get rustfmt validation)
- `edit_prepare` — prepare a search-and-replace edit (returns diff preview)
- `edit_confirm` — apply a previously prepared edit
- `search_files` — ripgrep search in the worktree
- `list_files` — fd file listing in the worktree
- `mate_send_update` — send a progress update to the captain
- `plan_create` — create the work plan before implementation
- `plan_step_complete` — mark a plan step as done, commits changes

### ACP built-in tool blocking

Both agents have ACP built-in tools (Bash, Read, Write, Edit) disabled:
- `crates/ship-core/src/acp_client.rs` — `create_terminal`, `read_text_file`, `write_text_file` return errors directing the agent to use MCP tools
- Permission requests for built-in tools are auto-rejected in `blocked_permission_option_id`
- `crates/ship-core/src/acp_driver.rs` — `ClientCapabilities` set `terminal: false` and `fs.read/write: false`

### Key crate responsibilities

- `ship-types` (`crates/ship-types/src/lib.rs`) — shared types: SessionEvent, TaskRecord, AgentState, ContentBlock, etc.
- `ship-service` (`crates/ship-service/src/lib.rs`) — RPC service trait definitions (roam)
- `ship-core` (`crates/ship-core/`) — session manager, ACP client/driver, git worktree ops
- `ship-server` (`crates/ship-server/`) — RPC server impl, captain/mate MCP servers, `listen` subcommand
- `frontend/` — React + Radix Themes UI, generated TypeScript bindings in `frontend/src/generated/`

### Generated code flow

`cargo xtask codegen` reads the roam service trait in `ship-service` and generates TypeScript client bindings at `frontend/src/generated/ship.ts`.

## Coordinator git safety

- Coordinator git operations that mutate repository state MUST be run sequentially, never in parallel
- This includes `git add`, `git commit`, `git merge`, `git rebase`, `git push`, and any command that can create `.git/index.lock`
- Parallel tool use is only allowed for read-only git inspection such as `git status`, `git log`, `git diff`, and `git show`
