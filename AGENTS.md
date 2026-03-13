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

Ship already created the current task worktree and manages workflow state internally. Do not use manual `git commit`, `git rebase`, `git merge`, or similar commands to advance task state.

- Captains research with `read_file` and `run_command`, delegate with `captain_assign`, steer with `captain_steer`, and finish review with `captain_accept` or `captain_cancel`.
- Mates checkpoint each completed step with `plan_step_complete`; that is the commit mechanism for task work.
- `run_command` and `read_file` already operate inside the current session worktree. Omit `cwd` by default, and do not pass repo-root paths or `.ship/...` prefixes unless the task explicitly targets a subdirectory inside the current worktree.

Stay within the scope of the assigned task. If a frontend task depends on a backend RPC, shared type, generated artifact, or other backend-owned surface that is not already present in your worktree, stop and report the missing dependency to the coordinator instead of implementing backend work from the frontend worktree. The same rule applies in reverse for backend tasks that depend on frontend work.

## Generated code

- Never hand-edit generated files.
- Change the source-of-truth inputs only.
- Then run `cargo xtask codegen` and include the regenerated artifacts in the same `plan_step_complete` checkpoint.
- If generated output looks wrong, fix the schema/codegen/runtime input that produced it rather than patching the output directly.

## Architecture: key files and layers

### Agent MCP tools

The captain and mate each get their own MCP server with a distinct set of tools.

**Captain** (`crates/ship-server/src/captain_mcp_server.rs`):
- `captain_assign` — assign a task to the mate (title + description)
- `captain_steer` — send direction to the mate on the current task
- `captain_accept` — accept the mate's submitted work and let Ship run the backend-managed rebase/merge flow
- `captain_cancel` — cancel the current task
- `captain_notify_human` — ask the human for guidance (blocks until response)
- `read_file` — inspect files in the current session worktree
- `run_command` — run read-only exploration commands in the current session worktree
- `web_search` — search the web when external context is required

The captain can inspect the current worktree, but Ship owns workflow state. Use git only for read-only inspection (`git status`, `git log`, `git diff`, `git show`), never for commits, rebases, or merges.

**Mate** (`crates/ship-server/src/mate_mcp_server.rs`):
- `run_command` — shell command via `sh -c` in the current session worktree
- `read_file` — read a file with line numbers, optional offset/limit
- `write_file` — write a file (Rust files get rustfmt validation)
- `edit_prepare` — prepare a search-and-replace edit (returns diff preview)
- `edit_confirm` — apply a previously prepared edit
- `mate_send_update` — send a progress update to the captain
- `set_plan` — create or revise the work plan when needed
- `plan_step_complete` — checkpoint a finished step; Ship commits that step for you
- `mate_ask_captain` — ask the captain for a decision or clarification
- `mate_submit` — submit finished work for captain review
- `web_search` — search the web when external context is required

The mate implements inside the current session worktree. Use `plan_step_complete` for checkpoint commits and do not run manual git workflow commands.

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
