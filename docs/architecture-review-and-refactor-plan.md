# Ship Architecture Review and Refactor Plan

## Executive Summary

Based on the reviewed files, Ship is not a clean end-to-end event-driven architecture today. It already has an event vocabulary, replay envelope, and state-application helpers, but production behavior is still orchestrated through shared mutable runtime state in `crates/ship-server/src/ship_impl.rs`.

The main structural risk is concentration of backend behavior in that file. `ship-core` already contains a partial session kernel, but `ship-server` still owns the live session map, startup flow, MCP coordination, and a meaningful amount of session mutation logic. That creates a hybrid design: part event-sourced model, part server-owned runtime object graph.

The highest-leverage first move is to establish one authoritative mutation path. Splitting `ship_impl.rs` earlier, or trying to mechanically remove mutexes first, would improve local shape without resolving the core ambiguity about where session behavior actually lives.

## Findings and Evidence

### Ship is currently a hybrid, not a clean end-to-end event-driven system

The event model is real. `ship-types` defines `SessionEvent`, `SessionEventEnvelope`, and `SubscribeMessage`, including append, patch, agent-state, task-state, and replay-oriented events (`crates/ship-types/src/lib.rs:474-566`). `ship-core` also exports state-application and replay helpers such as `apply_event`, `rebuild_materialized_from_event_log`, `coalesce_replay_events`, `transition_task`, and `set_agent_state` (`crates/ship-core/src/lib.rs:33-38`).

The spec describes a stronger architecture than the current runtime actually enforces. `docs/spec/backend.md` says every ACP notification should flow through one pipeline: translate, append to the event log, apply to materialized state, then broadcast (`docs/spec/backend.md:268-304`). The production server does not appear to route all mutation through one dedicated kernel boundary. Instead, `ship-server` imports the event helpers directly and calls them from many places inside `ship_impl.rs`, alongside direct session-map access and side effects (`crates/ship-server/src/ship_impl.rs:18-20`; `crates/ship-server/src/ship_impl.rs` matches at lines 776, 1101, 1147, 1255, 1311, 1745, 4108, 4468, 4476, 5117, 5209, 5263).

That is why the current design reads as hybrid rather than event-driven in the strict sense. The event model exists, but the server still behaves like the practical owner of session mutation.

### Backend behavior is concentrated in `ship_impl.rs`

`ShipImpl` owns project registry access, agent discovery, the ACP driver, worktree operations, persistence store, live sessions, pending MCP operations, server address state, startup timers, user avatar state, whisper model configuration, and the global event broadcaster (`crates/ship-server/src/ship_impl.rs:167-183`). That is a wide span of responsibilities for one runtime object.

The concentration is not only in fields. `ShipImpl` also implements the `Ship` service surface, the captain MCP surface, and the mate MCP surface in the same file (`crates/ship-server/src/ship_impl.rs:4725-5995`). The result is that transport adaptation, RPC entrypoints, session lifecycle, side effects, and session mutation all accumulate in one module.

There is also concrete duplication with `ship-core`. `SessionManager::create_session` already constructs and persists an `ActiveSession` in `ship-core` (`crates/ship-core/src/session_manager.rs:138-208`), while `ShipImpl::create_session` rebuilds essentially the same session record in `ship-server`, persists it, notifies listeners, and spawns startup work (`crates/ship-server/src/ship_impl.rs:4813-4924`). That duplication is a stronger architectural signal than file size alone: it shows that the system does not yet have one clear owner for backend session behavior.

### The spec and implementation have drifted

The backend spec says `ship-core` must hold the testability traits and the core session/task management logic, and that it must not depend on Tokio or another specific runtime (`docs/spec/backend.md:161-174`). The current crate split is looser than that.

`ship-core` does contain the traits and core helpers, but it also depends directly on Tokio (`crates/ship-core/Cargo.toml:11-30`). Meanwhile, `ship-server` still owns a substantial share of the session/task logic through `ShipImpl`, including the live session map and a duplicated session creation path (`crates/ship-server/src/ship_impl.rs:167-183`; `crates/ship-server/src/ship_impl.rs:4813-4924`).

This does not mean the split is wrong in every respect. It means the current implementation no longer matches the clean boundary described in the spec. Future refactoring should either restore the intended boundary or explicitly update the spec to describe the actual architecture.

### `ship-core` already contains the beginnings of a session kernel

`ship-core` is not empty scaffolding. It already exposes a generic `SessionManager<A, W, S>` over `AgentDriver`, `WorktreeOps`, and `SessionStore` (`crates/ship-core/src/session_manager.rs:121-136`). It owns `ActiveSession`, pending permission/edit state, startup state, persistence, and task/event helpers (`crates/ship-core/src/lib.rs:33-38`).

That matters because the right next step is not to invent a new architectural center. The beginnings of that center already exist. The missing piece is to make this path authoritative, so that `ship-server` stops competing with it as a second session-management implementation.

### The frontend duplicates projection logic

The frontend subscription path already distinguishes replay from live delivery. `useSessionState` buffers replay events until `ReplayComplete`, dispatches them as a `replay-batch`, and then switches to per-event updates for live traffic (`frontend/src/hooks/useSessionState.ts:214-337`).

`sessionReducer` then implements separate replay and live application paths. The `replay-batch` branch mutates cloned block stores and manually updates task, agent, startup, and title fields (`frontend/src/state/sessionReducer.ts:127-274`), while the `event` branch repeats the same conceptual updates one event at a time (`frontend/src/state/sessionReducer.ts:276-475`).

This is avoidable duplication. It is easy for one path to learn a new event or edge case slightly differently than the other. The current code is understandable, but it is carrying two projection models where one would be safer.

## Why This Feels Like a Soup of Mutexes

The phrase is not a claim that events are absent. The event model is present in the types and in part of the backend implementation. The problem is that the runtime still feels organized around ownership of mutable containers rather than around one authoritative mutation boundary.

A large part of that feeling comes from `ShipImpl` itself. The top-level server object keeps many `Arc<Mutex<...>>` or `Arc<tokio::sync::Mutex<...>>` fields for sessions, pending MCP operations, server URLs, startup timers, optional UI metadata, and other runtime state (`crates/ship-server/src/ship_impl.rs:167-183`). That would be less concerning if those mutexes mostly wrapped caches or transport details. Instead, they still sit close to meaningful session behavior.

The second contributor is that `ship_impl.rs` mixes several layers in one place: service entrypoints, session state mutation, side effects, startup sequencing, MCP coordination, persistence triggers, and event broadcasting (`crates/ship-server/src/ship_impl.rs:4725-5995`). When mutation and orchestration live together like this, locks become the visible architecture, even if the data being protected is event-oriented.

The third contributor is duplication between the kernel-shaped code in `ship-core` and the production code in `ship-server`. Once there are two plausible places to mutate session state, the codebase stops reading like an event pipeline and starts reading like a runtime that happens to use events.

In short: the issue is not that Ship chose mutexes. The issue is that mutex-protected server state is still one of the practical centers of authority.

## Target Architecture

### `ship-core`

_TODO: define `ship-core` as the authoritative session kernel for session mutation, event application, task transitions, persistence-facing state, and replay helpers._

### `ship-server`

_TODO: define `ship-server` as the transport, process, IO, and side-effect boundary that delegates session mutation to `ship-core`._

### Frontend projection layer

_TODO: define the frontend as a single event projection layer with one authoritative event application path for both replay and live updates._

## Phased Refactor Plan

### Phase 1: Define invariants and architectural rules

_TODO: specify the rules that future changes must preserve._

### Phase 2: Consolidate session behavior into a real kernel in `ship-core`

_TODO: move the authoritative mutation path into `ship-core` instead of duplicating it in `ship-server`._

### Phase 3: Separate command handling from side effects

_TODO: distinguish pure state transitions from IO, process management, and broadcast work._

### Phase 4: Make `ship-server` delegate

_TODO: reduce `ship-server` to orchestration, transport adaptation, and side-effect execution around kernel decisions._

### Phase 5: Split `ship_impl.rs` by responsibility after logic is centralized

_TODO: only decompose files once the logic boundary is real._

### Phase 6: Unify frontend event application paths

_TODO: remove replay/live reducer duplication and converge on one projection model._

### Phase 7: Resume feature work after the architecture is stable

_TODO: describe the threshold for returning to normal feature delivery._

## What Success Looks Like

_TODO: describe the observable technical end state for backend mutation flow, server delegation, frontend projection, and spec alignment._
