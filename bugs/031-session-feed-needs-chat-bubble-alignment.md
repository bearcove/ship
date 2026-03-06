# 031: Session feed should render user and captain messages as distinct chat bubbles

Status: open
Owner: frontend

## Symptom

Captain and user messages are rendered as stacked cards/text blocks without clear conversational alignment. User-submitted steer text is hard to recognize in the feed.

## Expected behavior

- Captain/agent messages should read as left-side feed/chat items.
- User steer messages should read as right-side feed/chat items.
- Message ownership should be obvious at a glance.

## Evidence

- Screenshot from March 6, 2026 shows `You` and `CAPTAIN` styling that does not create a clear left/right conversational model.
- User report: submitted captain steer does not clearly appear in the feed.

## Suspected root cause

The session feed still mixes status blocks and message blocks without a strong conversation layout model.

## Spec impact

- `r[view.session]`
- `r[ui.block.text]`
- `r[ui.layout.session-view]`

## Next action

- Introduce a clear conversation layout for user vs captain messages.
- Keep tool/status blocks readable without losing the chat structure.
