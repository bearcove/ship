# 029: Captain greeting waits on full session startup instead of greeting early

Status: open
Owner: fullstack

## Symptom

Session startup reports `initialize ACP connection` quickly, but the captain greeting does not begin until roughly 30+ seconds later.

## Expected behavior

The captain should greet the user as soon as the captain session is ready. Startup of the mate should not block the first captain interaction.

## Evidence

- Server log from March 6, 2026 shows:
  - mate ACP connection initialized around `19:01:27`
  - mate ACP session started around `19:01:29`
  - only then does startup move to `GreetingCaptain`
  - `elapsed_ms=34593`
- User report: session appears idle for about 30 seconds before the first captain message appears.

## Suspected root cause

Session startup still treats captain greeting as a late startup stage after both agents are fully initialized, instead of letting the captain become user-facing as soon as the captain side is ready.

## Spec impact

- `r[session.create]`
- `r[captain.initial-prompt]`
- `r[view.session]`

## Next action

- Let the captain greet as soon as the captain ACP session is ready.
- Decouple mate readiness from the first captain/user interaction.
- Keep startup progress for the mate visible separately if it is still coming up in the background.
