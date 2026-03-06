# 019: Server hardcodes the captain/mate workflow

Status: open
Owner: fullstack

## Symptom

The server currently orchestrates the captain and mate as a fixed workflow:
- prompt captain for implementation direction
- prompt mate to execute
- prompt captain to review mate output

This happens automatically, even for a simple request like “just say hi.”

## Expected Behavior

The captain should be long-lived and user-facing.

The server should not automatically prompt the mate during session creation or run a hardcoded captain-review loop. Instead:
- the captain gets role/bootstrap context
- the captain greets the user and waits
- the user steers the captain
- the captain decides when to delegate to the mate
- delegation/review should be explicit, not baked into a fixed backend pipeline

## Evidence

Current server flow:
- `run_task_prompt_flow(...)` prompts the captain
- then transitions the task to `Working`
- then prompts the mate
- `handle_mate_stop_reason(...)` marks review pending and prompts the captain to review

Relevant code:
- `crates/ship-server/src/ship_impl.rs`

## Suspected Root Cause

Workflow orchestration is embedded in backend task-control code instead of being driven by the captain over time.

## Spec Impact

Captain role semantics, mate delegation semantics, review flow semantics, and overall session interaction model.

## Next Action

- remove the fixed captain->mate->captain orchestration from backend task startup
- replace it with a long-lived captain-driven interaction model
- introduce an explicit mechanism for captain-to-mate delegation rather than an automatic mate prompt
