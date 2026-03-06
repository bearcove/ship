# 018: Session creation implicitly starts a task

Status: open
Owner: fullstack

## Symptom

Creating a session immediately creates `current_task`, marks it `Assigned`, and starts task execution logic before the user has spoken to the captain.

This makes a brand-new session behave like a one-shot task runner instead of a long-lived conversation with the captain.

## Expected Behavior

Creating a session should create durable session state and navigate immediately.

A new session should not implicitly create a task. The captain should come up, greet the user, and wait for user input. Task creation should happen later, when the user or captain explicitly decides work should start.

## Evidence

Current server flow:
- `create_session(...)` calls `start_task(...)`
- `start_task(...)` creates `current_task` and emits `TaskStarted` / `TaskStatusChanged`

Relevant code:
- `crates/ship-server/src/ship_impl.rs`

## Suspected Root Cause

The current implementation models a session as “a task with two agents attached” instead of “a long-lived captain-led conversation that may later create tasks.”

## Spec Impact

Session lifecycle, task lifecycle, captain bootstrap behavior, and session-creation UX.

## Next Action

- decouple session creation from task creation
- make new sessions start with no `current_task`
- model captain bootstrap/greeting separately from task assignment
