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

`ship-core` should become the authoritative session kernel. It should own the durable session state model, task state machine, event-log mutation rules, materialized-state updates, replay helpers, and the validation of allowed transitions. In practical terms, backend code outside `ship-core` should stop constructing or mutating `ActiveSession` directly.

That does not require `ship-core` to perform IO itself. It requires `ship-core` to be the only place where backend session state is decided. If an operation changes task state, appends an event, resolves a permission, starts or ends a review, or updates agent-facing session state, the authoritative state transition should be expressed in `ship-core` first.

### `ship-server`

`ship-server` should become the adapter layer around the kernel. Its job should be transport handling, ACP process lifecycle, filesystem and git integration, WebSocket and roam wiring, HTTP concerns, and executing side effects that the kernel requests.

In that design, `ship-server` still matters, but it stops being a competing implementation of session management. RPC and MCP handlers should translate requests into kernel operations, execute any required side effects, persist or broadcast as required by the kernel contract, and then return results. It should not re-encode the session state machine inline.

### Frontend projection layer

The frontend should have one authoritative event application path. Replay and live delivery can remain operationally distinct at the subscription level, but both should converge on the same projection logic so that every `SessionEvent` is interpreted exactly once.

Concretely, that means the reducer should have a shared event-application function that can be folded over a replay batch and also applied to a single live event. The frontend should continue to store derived UI state, but it should not maintain separate semantic implementations for replay and live mutation.

## Phased Refactor Plan

### Phase 1: Define invariants and architectural rules

Write down the rules the refactor is meant to enforce before moving code. At minimum:

- Backend session state has one authoritative mutation path.
- Event-log append, materialized-state update, persistence update, and subscriber broadcast have a defined ordering contract.
- `ship-server` may execute side effects, but it does not invent new session transitions outside the kernel.
- The frontend applies each event through one semantic projection path, regardless of replay or live delivery.

This phase should also decide whether the spec is the source of truth to restore, or whether the intended target architecture needs a spec update before implementation proceeds further.

### Phase 2: Consolidate session behavior into a real kernel in `ship-core`

Move duplicated session behavior out of `ship-server` and into `ship-core` until `ship-core` is the undisputed owner of backend session mutation. The immediate targets are the operations currently duplicated or partially split, such as session creation, startup-state transitions, task lifecycle transitions, permission state, human review state, and event-log mutation.

This does not need to happen as one giant rewrite. A practical approach is to move one vertical slice at a time behind kernel entrypoints while preserving the existing transports. The key requirement is that each migrated slice ends with fewer state mutations in `ship_impl.rs`, not just different helper names.

### Phase 3: Separate command handling from side effects

Once the kernel owns mutation, separate pure state decisions from effect execution. Kernel operations should decide what state changes and which follow-up effects are required, while the server executes those effects.

The exact representation can vary, but the outcome should look like this: the kernel determines state transitions and follow-up intents; the server performs ACP calls, git/worktree operations, persistence writes, and event broadcasts; then control returns without re-deriving state in the server layer. This is the step that makes the architecture more than a file move.

### Phase 4: Make `ship-server` delegate

After the kernel boundary is real, simplify `ShipImpl` and related server code so that request handlers delegate rather than mutate. `Ship` RPC methods, captain MCP tools, and mate MCP tools should mostly validate inputs, call the kernel, execute requested effects, and translate results back to protocol responses.

This is also the point where server-owned maps and mutexes should become easier to reason about. Some shared runtime state will still exist, but it should mostly support transport and effect coordination rather than hold the session state machine together.

### Phase 5: Split `ship_impl.rs` by responsibility only after logic is centralized

Only after the previous phases are in place should `ship_impl.rs` be split. At that point, file boundaries can reflect real responsibility boundaries instead of slicing a still-entangled implementation into smaller entangled files.

A reasonable outcome would be separate modules for service handlers, session-runtime orchestration, MCP tool services, and transport/bootstrap code. The important rule is that the split follows architectural seams that already exist in behavior, not wishful seams created by moving methods around.

### Phase 6: Unify frontend event application paths

Refactor the frontend reducer so that replay and live updates share one event-application implementation. A replay batch should be a fold over the same event handler used for live messages, with batching only as a performance concern.

This phase should also remove any event coverage gap between the replay and live branches and make it difficult to add a new backend event without updating the shared projection function. The goal is not only smaller code. The goal is a single semantic contract for UI state projection.

### Phase 7: Resume feature work after the architecture is stable

Normal feature delivery should resume only after the kernel boundary is established, the main backend duplication is removed, and the frontend no longer has separate replay/live semantics. Until then, feature work in the same area risks cementing the current split-brain design.

This does not require every file to be perfect. It requires the major sources of architectural ambiguity to be closed first, so that new work lands on the stable path instead of extending both paths.

## What Success Looks Like

Success is not merely a smaller `ship_impl.rs`. It is a system where the architectural center is obvious from the code.

Signs that the refactor succeeded:

- Backend session creation, task transitions, permission changes, review state, and event-log mutation are authored through `ship-core` rather than reimplemented in `ship-server`.
- `ship-server` no longer constructs or mutates core session state directly except through the kernel boundary.
- The ordering between event append, state application, persistence, and broadcast is explicit and consistently enforced.
- The frontend uses one event-application path for replay and live updates.
- The spec and crate boundaries agree closely enough that future work does not require guessing which layer owns session behavior.

If those conditions are true, Ship will still use mutable runtime state and probably still use some mutexes. That is fine. The architecture will feel materially cleaner because the mutation authority will be singular and legible.
