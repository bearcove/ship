# 027: Structured tool-call schema drift breaks session subscription

Status: open
Owner: fullstack

## Symptom

After session startup begins, the client can disconnect with a deserialization error while reading `BlockAppend.block.ToolCall.value`:

```text
Unknown schema kind: [object Object]
```

The session then enters reconnect churn instead of continuing to stream events.

## Expected behavior

Structured tool-call events should decode cleanly on the client. Changing the tool-call payload model must not break live session subscription.

## Evidence

- Screenshot from March 6, 2026 shows:
  - `Connection lost — attempting to reconnect`
  - `subscription setup failed: deserialize error`
  - path `Event.value.event.BlockAppend.block.ToolCall.value`
  - last event `BlockPatch at seq 40`

## Suspected root cause

The richer structured `ToolCall` payload shape and the generated/client schema are out of sync somewhere in the live subscription path.

## Spec impact

- `r[acp.content-blocks]`
- `r[event.subscribe]`
- `r[proto.hydration-flow]`

## Next action

- Reproduce with the current generated schema.
- Compare server-side encoded `ToolCall` event shape to the frontend/runtime schema used during live subscription.
- Add an end-to-end regression test for `BlockAppend(ToolCall)` decoding over the real client path.
