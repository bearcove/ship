# 020: Captain MCP tools are specified but not implemented

Status: open
Owner: fullstack

## Symptom

The intended captain-driven workflow cannot exist yet because Ship does not
actually expose the captain MCP tools described in the spec.

Today the backend hardcodes a captain -> mate -> captain review pipeline
internally instead of letting the captain call back into Ship to:
- steer the mate
- accept work
- reject/cancel work

## Expected Behavior

Ship should expose MCP tools to the captain so the captain can drive the
workflow explicitly:
- `ship_steer(message)`
- `ship_accept(summary?)`
- `ship_reject(reason)`

Those tools should be exposed through Ship's own MCP server transport for the
captain session, not simulated by a fixed backend prompt sequence.

## Evidence

Spec references:
- `r[captain.tool.steer]`
- `r[captain.tool.accept]`
- `r[captain.tool.reject]`
- `r[captain.tool.implementation]`
- `r[captain.tool.transport]`

Current code only has:
- normal Ship RPCs like `steer`, `accept`, and `prompt_captain`
- backend-owned orchestration in `run_task_prompt_flow(...)`

There is no implementation of:
- a per-captain MCP server
- `ship_steer`
- `ship_accept`
- `ship_reject`
- the stdio MCP proxy transport described by the spec

## Suspected Root Cause

The implementation stopped at direct backend orchestration and never built the
captain-side MCP callback path that the spec requires.

## Spec Impact

Captain role semantics, delegation semantics, review semantics, and session
workflow architecture.

## Next Action

- implement Ship-hosted MCP tools for the captain
- expose them to the captain session via the stdio transport described in the spec
- remove the hardcoded backend captain -> mate -> captain orchestration in favor
  of explicit captain tool calls
