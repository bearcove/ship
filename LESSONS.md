# Lessons from mate (the predecessor)

Ship replaces a tmux-based tool called `mate`. These are the hard-won lessons.

## What worked

- **Captain/mate model** — two agents with distinct roles is the right
  abstraction. The captain stays high-level, the mate stays hands-on.
  Don't let them swap roles.
- **Roam RPC** — trait-based service definition with typed
  request/response. Generated TypeScript clients. No schema drift.
- **One active task per session** — simplicity that prevents confusion.
  Queue the next task, don't multiplex.
- **Task lifecycle with explicit accept** — assign, update, steer, respond,
  accept, cancel. Every state transition is intentional. No implicit completion.
- **Idle detection over staleness detection** — "the agent says it's done" beats
  "the terminal output hasn't changed in 2 minutes." Staleness is a broken
  heuristic. ACP gives us `StopReason` which is the real thing.

## What didn't work

- **Terminal scraping for agent state** — parsing Claude's spinner characters
  and Codex's "Working (35s)" from captured terminal output. Fragile,
  agent-version-dependent, untestable. ACP eliminates this entirely.
- **Emoji paste markers** — sending 3 random emoji as a marker then polling
  `capture-pane` to detect when the paste landed. Horrible.
- **Stringly-typed everything** — pane IDs as `String`, session names as
  `String`, context remaining as `Option<String>` holding "98% left".
  Use newtypes: `SessionId`, `TaskId`, `AgentKind`, `Role`.
  Context is `Option<u8>` (percentage, computed at parse time).
- **Sending `/clear` to reset agent state** — a command that might or might
  not work depending on what mode the agent is in. ACP sessions are
  proper state machines.
- **Filesystem as database** — request state scattered across
  `/tmp/mate-requests/`, `/tmp/mate-responses/`, `/tmp/mate-idle/`,
  `/tmp/mate-orphaned/`. Implicit schema. Use a proper data structure
  in memory, persist with a known format.
- **Mixing formatting with delivery** — `send_to_pane` received the fully
  formatted message including instructions like
  `cat <<'EOF' | mate steer <id>`. The delivery layer should not know
  about message content. In Ship, the backend constructs ACP prompts;
  the transport just delivers them.

## Architecture principles for Ship

- **Traits at boundaries** — agent communication, task storage, git operations.
  Each gets a trait. Production impls talk to real systems. Test impls are
  in-memory. No subprocesses in tests.
- **Newtypes for identifiers** — `SessionId`, `TaskId`, not strings. The
  compiler catches misuse.
- **Events, not polling** — ACP pushes notifications. The backend pushes
  `SessionEvent`s to the frontend via roam streams. Nobody polls.
- **Agent-agnostic core** — the session/task logic doesn't know if it's
  talking to Claude or Codex. The ACP adapter handles agent-specific
  differences.

## Stack

- Backend: Rust, roam (RPC), ACP client (agent-client-protocol crate)
- Frontend: TypeScript, roam-codegen'd types
- Agent control: ACP (lib.rs/crates/agent-client-protocol)
- Worktrees: git, managed by the backend
