# Session startup status belongs in the feed, not the top bar

## Symptom

During session startup, Ship shows a global banner at the top saying `Session startup is in progress.`

## Expected behavior

Startup status should appear inline in the captain's feed, where the conversation and session activity already live. It should not occupy top-level global chrome.

## Owner

`frontend`

## Evidence

- Session view screenshot from March 6, 2026 shows the startup message as a full-width banner at the top of the page while both panels are otherwise empty.

## Suspected root cause

Startup state is currently rendered through top-level session-view chrome instead of being treated as feed-local session activity.

## Spec impact

- `r[ui.layout.session-view]`
- `r[view.session]`

## Next action

- Move startup-progress messaging into the captain panel/feed area.
- Keep the session header focused on navigation and controls.
