# Haiku-driven mate status line

## Idea

While the mate is working, show a short human-readable status phrase in the thinking bubble (e.g. "Running cargo check", "Editing auth module") instead of just token/tool counts.

## How it works

1. As the mate produces `AgentMessage` text blocks (via `mate_send_update`), pipe those to Haiku
2. Haiku distills the recent mate speech into a short current-status string (1 short sentence or phrase)
3. A new session event carries the status string to the frontend
4. The frontend displays it in the mate's thinking bubble

## Why Haiku

- Fast and cheap — called on every mate text block to keep status fresh
- The input is small (recent mate speech only, not tool outputs)
- Output is a single short phrase — well within Haiku's strengths

## Backend work needed

- Call Haiku when a mate AgentMessage block is committed, passing recent mate speech as context
- Emit a new `AgentStatusChanged` (or similar) event with `role` + `status: String`
- Rate-limit or debounce if needed (e.g. at most once per 2s)

## Frontend work needed

- Handle the new event in `useSessionState`
- Pass the status string down to `UnifiedFeed`
- Display it in the mate's thinking bubble (replaces or complements the token/tool counts)
