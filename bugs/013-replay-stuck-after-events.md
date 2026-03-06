# 013: Session view can stay stuck in replay mode after events arrive

Status: open
Owner: fullstack

## Symptom

The session view can remain in replay mode even after many events have already arrived and been applied.

Examples:
- banner stays on `Connected — replaying events (...)`
- panel label stays on `Replaying N events...`
- feed content is already visible, but the UI never transitions to the normal live state

## Expected Behavior

Once replayed events have been delivered and the replay completion marker is received, the client should leave replay mode and transition to live mode.

## Evidence

User screenshot and console logs showing:
- replay event count increasing
- events being received and applied
- UI still showing `Replaying 16 events...`

## Suspected Root Cause

Likely a fullstack replay lifecycle bug:
- `ReplayComplete` may not be sent in some paths
- or it may be sent but not processed correctly by the client
- or the replay/live phase state may be reset by some later event/connection transition

## Spec Impact

Hydration and replay lifecycle requirements, plus session-view connection UX.

## Next Action

- verify `ReplayComplete` is always sent after replay
- verify the client always processes it exactly once
- add end-to-end logging around replay completion and phase transitions
