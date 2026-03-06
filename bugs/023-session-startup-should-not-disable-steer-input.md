# Session startup should not disable steer input

## Symptom

While session startup is still running, the `Steer the captain directly...` input is disabled.

## Expected behavior

The user should still be able to type into the steer input during startup. At minimum, sending can be disabled while preserving editing. Better behavior would be to queue the message for send once startup is ready.

## Owner

`fullstack`

## Evidence

- Session view screenshot from March 6, 2026 shows both inline steer composers disabled during startup with the status text `Session startup is still in progress.`

## Suspected root cause

Composer disabled state is keyed directly off startup readiness, which blocks both text entry and submit rather than separating editing from dispatch.

## Spec impact

- `r[ui.task-bar.actions]`
- `r[ui.layout.session-view]`

## Next action

- Allow typing during startup.
- Either:
  - disable only submit until startup is ready, or
  - queue submitted steer text until startup completes.
- Make the chosen behavior explicit in UI copy and tests.
