# 028: Session feed does not render the conversation model clearly

Status: open
Owner: frontend

## Symptom

Even when startup eventually succeeds:
- the captain greeting is rendered as loose text instead of a clear chat/feed item
- the progress bar appears without clear meaning
- user steer messages are not shown back in the feed in an obvious way

This makes the session look like a status dashboard instead of an actual conversation.

## Expected behavior

The captain greeting, startup progress, user steer messages, and follow-up responses should read as feed items with clear conversational structure.

## Evidence

- Screenshot from March 6, 2026 shows the captain greeting as plain text under a progress bar with no bubble/feed framing.
- User report from the same session says sent steer text does not show up anywhere obvious in the UI.

## Suspected root cause

The new captain-led lifecycle is only partially reflected in the feed rendering. Status UI and conversation UI are still mixed together in a way that hides message boundaries and who-said-what.

## Spec impact

- `r[view.session]`
- `r[ui.layout.session-view]`
- `r[ui.block.text]`

## Next action

- Render startup/progress as explicit feed items instead of floating chrome.
- Make user steer messages visible in the session feed.
- Clarify or replace the current progress bar unless it has a well-defined user-facing meaning.
