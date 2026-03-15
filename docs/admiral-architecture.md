# Admiral Architecture

## Problem

The admiral is currently shoehorned into the session model designed for
captain+mate pairs. It shows up as a broken session thread with empty
agent slots, invalid dates, and messages that go nowhere. Previous
attempts built a separate UI (event list) that didn't match the rest of
the app.

## Principle: same UI, flexible model

The admiral session should look exactly like a regular chat thread from
the user's perspective. They're just talking to someone. The data model
can differ internally, but the session concept should be flexible enough
to accommodate both cases.

## What the admiral is

A single persistent agent that sits above all sessions:

- **One agent** — no captain/mate split, no summarizer, no tasks, no plans.
- **Not project-scoped** — the admiral sees across all projects and sessions.
- **Persistent** — survives across sessions, maintains long-running context.
- **Cross-session routing** — receives `@admiral` mentions from any captain
  in any session, and can `@captain` reply back to specific sessions.

## What the admiral is not

- Not a session with two agents and empty slots
- Not a special UI with its own rendering
- Not a project-level entity

## How it should work

### From the human's perspective

The admiral appears as a chat thread in the sidebar, like any other
session. The human opens it and types messages. The admiral responds.
No plan panel, no agent status chips (or just one chip for the admiral
itself). Same composer, same message rendering, same feel.

### From the system's perspective

A session has a **session kind** that determines its shape:

- **Task session** (current model): captain + mate + summarizer, has
  tasks/plans/commits. Operates in an isolated worktree branch.
- **Admiral session**: single agent, no tasks, receives cross-session
  mentions. No worktree needed.

The session model gains flexibility rather than the admiral getting a
separate system. This means `ActiveSession` needs to support optional
fields (mate is `None`, current_task is `None`, etc.) or the session
kind gates which fields are present.

### Worktree flexibility

Sessions should support different worktree modes:

- **Isolated worktree** (current task session default): a separate git
  worktree branched from main. Changes merge back to main via
  captain_merge.
- **Main tree**: the session works directly in the main worktree. Useful
  for iterating on things in-place without branch overhead.
- **No tree**: an empty temp directory or no filesystem context at all.
  For the admiral or pure conversation sessions.

### Merge model

Currently all task sessions merge locally back to main on the same
machine. A pull request model is planned for later — the session would
push to a remote branch and open a PR instead of merging locally. The
session model should not assume local merge is the only path.

### Routing

- Captain says `@admiral` in any session → message routed to the
  admiral's conversation as a new human-role message with attribution
  (which session/captain it came from).
- Admiral says `@captain(session_id)` or similar → message routed back
  to that specific captain's feed.
- Human messages to the admiral → direct, like talking to a captain.
- Admiral says `@human` → visible in the admiral's thread (human is
  already reading it).

### Event stream

The admiral session uses the same `SessionEvent` enum and event stream.
`BlockAppend`, `BlockPatch`, `BlockFinalized`, `AgentStateChanged` all
work the same way. The frontend renders them identically.

Events that don't apply (TaskStarted, TaskStatusChanged, ChecksStarted,
etc.) simply never fire in an admiral session.

## What needs to change

### Backend

1. **Session kind** — add a discriminator to `ActiveSession` or session
   config that distinguishes task sessions from admiral sessions.
2. **Role::Admiral** — add to the enum, with client capabilities.
3. **Admiral lifecycle** — the admiral session is created once (on first
   use or server start) and persists. Not tied to any project.
4. **Cross-session mention routing** — when `RouteToAdmiral` fires in
   any session's `drain_notifications`, deliver the message to the
   admiral session's event stream.
5. **Admiral → captain routing** — parse `@captain` mentions from the
   admiral's output and route to the correct session.

### Frontend

1. **Sidebar** — admiral session appears as a thread, possibly pinned
   at the top or in its own section.
2. **Session view** — hide plan panel, task status, mate agent chip
   when session kind is admiral. Show single agent chip.
3. **Composer** — same composer, no `@mate` target parsing needed
   (admiral sessions don't have mates).

## Open questions

- How does the admiral reference specific sessions when replying?
  Session IDs are opaque. Maybe session titles or a shorthand.
- Should the admiral have tools? (read files, run commands, web search)
  Probably yes for web search at minimum.
- How does persistence work across server restarts? The admiral's
  conversation history needs to survive.
- Should the human be able to create multiple admiral sessions or is
  there exactly one?
