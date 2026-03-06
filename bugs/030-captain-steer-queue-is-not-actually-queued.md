# 030: Captain steer during startup is not queued; it races into `prompt already in flight`

Status: open
Owner: fullstack

## Symptom

The user can type into `Steer the captain directly`, but submitting during startup does not show up in the feed and does not reliably reach the captain. Server logs show:

```text
ship_impl call failed action=prompt_captain error=prompt already in flight
```

## Expected behavior

If startup/initial greeting is still in flight, user steer should either:
- queue behind the current captain prompt, or
- be visibly rejected with a clear reason

It must not silently disappear.

## Evidence

- User report from March 6, 2026: `for now just say hi to me` did not show up anywhere and did not reliably affect captain behavior.
- Server log shows a second `starting agent prompt` for Captain immediately followed by `prompt already in flight`.

## Suspected root cause

The UI/backend path claims startup steer can be queued, but the implementation still calls `prompt_captain` directly while the greeting prompt is active.

## Spec impact

- `r[captain.initial-prompt]`
- `r[view.session]`
- `r[ui.layout.session-view]`

## Next action

- Implement real prompt queuing for captain steer during startup/active captain prompt.
- Or explicitly reject with visible UI state until queuing exists.
- Ensure submitted steer appears in the feed either way.
