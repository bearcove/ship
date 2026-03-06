# 026: Session startup is taking timeout-scale latency

Status: open
Owner: fullstack

## Symptom

Session startup can sit in `Session startup is in progress.` for roughly 30 seconds before the captain finally appears. The delay feels suspiciously close to a call-timeout boundary rather than normal ACP startup time.

## Expected behavior

ACP/session startup should complete promptly, or at least stream meaningful progress without sitting on a long opaque delay that looks like timeout behavior.

## Evidence

- User report from March 6, 2026: startup took about 30 seconds before the captain greeting appeared.
- This happens in the same area where session startup currently shows `Starting` composers and the startup banner/feed state.

## Suspected root cause

There may still be one or more unary/RPC lifetime mistakes or blocking startup steps that line up with a 30-second timeout window.

## Spec impact

- `r[session.create]`
- `r[proto.create-session]`
- `r[view.session]`

## Next action

- Instrument the startup stages with elapsed timing.
- Identify whether any startup sub-step is still waiting on a timeout-scale RPC boundary.
- Make startup progress explicit in the session feed while the latency issue is being fixed.
