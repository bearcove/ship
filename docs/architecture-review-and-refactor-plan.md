# Ship Architecture Review and Refactor Plan

## Executive Summary

_TODO: summarize the current architecture state, the main structural risk, and the recommended first move._

## Findings and Evidence

### Ship is currently a hybrid, not a clean end-to-end event-driven system

_TODO: document the event model that exists in `ship-types` and `ship-core`, and where production orchestration still bypasses a single event pipeline._

### Backend behavior is concentrated in `ship_impl.rs`

_TODO: document the concentration of runtime behavior in `crates/ship-server/src/ship_impl.rs` and why that raises change risk._

### The spec and implementation have drifted

_TODO: compare the responsibilities described in `docs/spec/backend.md` with the current `ship-core` and `ship-server` split._

### `ship-core` already contains the beginnings of a session kernel

_TODO: document the responsibilities already present in `SessionManager` and the related event/state helpers in `ship-core`._

### The frontend duplicates projection logic

_TODO: document the separate replay and live event application paths and the maintenance risk they create._

## Why This Feels Like a Soup of Mutexes

_TODO: explain why the runtime feels mutex-driven even though the type system and parts of the architecture already model events and replay._

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
