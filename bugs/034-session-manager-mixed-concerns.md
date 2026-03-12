# `session_manager.rs` mixes session management with event processing functions

`crates/ship-core/src/session_manager.rs` is 1584 lines. The bottom ~450 lines (from line ~1083 onward) are standalone public functions with no dependency on the `SessionManager` struct:

- `apply_event` — applies a `SessionEvent` to an `ActiveSession`
- `apply_event_to_materialized_state` — updates materialized view from event
- `apply_block_patch` — patches a `ContentBlock`
- `rebuild_materialized_from_event_log` — full rebuild from log
- `transition_task` — validates and applies task status transitions
- `is_valid_transition` — transition validity table
- `archive_terminal_task` — moves completed tasks to archive
- `current_task_status` — reads current task status
- `set_agent_state` — updates agent state
- `coalesce_replay_events` — deduplicates replay events

These are pure functions over session state and belong in a separate `event_processing.rs` (or `session_state.rs`) module. Their current placement makes `session_manager.rs` harder to navigate and the `SessionManager` struct harder to understand in isolation.

## Fix

Move the standalone event/state functions to `crates/ship-core/src/session_state.rs` and re-export them from `lib.rs` as needed.
