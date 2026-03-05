# 001: ACP usage update decode mismatch

Status: open
Owner: backend

## Symptom

When Claude is used as an agent, the server logs ACP decode failures for `usage_update`.

## Expected Behavior

The ACP client should accept usage/context updates from the agent and convert them into Ship state updates instead of logging decode errors.

## Evidence

Server log excerpt:

```text
ERROR agent_client_protocol::rpc: failed to decode ... "unknown variant `usage_update`, expected one of `user_message_chunk`, `agent_message_chunk`, `agent_thought_chunk`, `tool_call`, `tool_call_update`, `plan`, `available_commands_update`, `current_mode_update`, `config_option_update`"
```

## Suspected Root Cause

Ship's ACP schema/runtime is behind the Claude ACP payload shape and does not recognize the newer `usage_update` session update variant.

## Spec Impact

Likely affects context usage reporting and related UI/state requirements.

## Next Action

- Check the ACP crate/version and update support for `usage_update`.
- Decide whether this requires a dependency bump, feature flag, or local compatibility shim.
- Add a regression test if the ACP layer is testable with canned payloads.
