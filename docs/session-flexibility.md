# Session Flexibility

## Current state

Sessions are hardcoded around the captain+mate+summarizer model with an
isolated worktree, task tracking, and local merge. The admiral was bolted
on as a special case and doesn't work.

## Direction

Instead of hardcoding session kinds (task vs admiral), sessions should be
composed from capabilities:

### Agents

A session has a list of agents. No fixed roles — just agents with
different personas and capabilities:

- A task session happens to have: captain, mate, summarizer
- An admiral session happens to have: admiral
- A quick-question session might have: just a captain
- The summarizer is an agent, not a session feature

### Infrastructure (optional)

- **Worktree**: isolated branch (current default), main tree (in-place),
  or none (pure conversation)
- **Task tracking**: plans, steps, commits, check runs — or none
- **Merge strategy**: local merge to main (current), push + PR (future),
  or none

### Implications

- `ActiveSession` fields like `mate_handle`, `current_task`, etc. become
  optional based on what the session was composed with, not based on an
  enum discriminator.
- Event triggers (TaskStarted, ChecksStarted, etc.) only fire when the
  relevant infrastructure is present.
- The UI adapts to what's present: no plan panel if no task tracking, no
  mate chip if no mate agent, etc.
- New session configurations don't require new enum variants or special
  cases — just compose differently.

## Relationship to other docs

- `docs/admiral-architecture.md` — the admiral is one configuration of
  a flexible session (single agent, no worktree, no tasks).
- `docs/interaction-model.md` — @mention routing works the same
  regardless of session configuration. Agents mention each other by name.
