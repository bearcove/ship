# Native iPhone Client Plan

## Executive Summary

A native iPhone client for Ship is viable now. Ship already has the important backend pieces: a typed RPC surface, per-session event streaming with replay, a frontend reducer model that defines the live session view, and an existing audio transcription endpoint. The iPhone app is therefore mostly a native client layer over current backend capabilities, not a new backend initiative.

A useful prototype should be achievable in about a day: connect, list sessions, open one session, replay the feed, and send text prompts or steers. An internal MVP should fit in roughly a week. A polished app takes longer mostly because of UI refinement, reconnect/replay correctness, notifications, and audio-session edge cases.

## Scope Assumptions

- Internal app first, for the team that already runs Ship.
- Not App Store-grade by default.
- No offline-first sync, multi-account support, or push infrastructure in the first pass.
- Keep the Rust backend as the source of truth for session state, task state, and event sequencing.

## Recommended Direction

Build a SwiftUI-first iPhone app and only drop to UIKit where SwiftUI is awkward, most likely for a wrapped `UITextView` composer with better multiline editing, mentions, and transcript insertion behavior.

The client should mirror the current web architecture:

- One transport layer for the Ship roam WebSocket connection.
- A thin typed client for key `Ship` RPCs.
- A session-view store driven by the same two-phase model the web client uses: `get_session` for structure, then `subscribe_events` for replay plus live updates.
- SwiftUI views over that derived state.
- Optional voice input UI layered over the existing `transcribe_audio` RPC.

## Why This Is Mostly A Client

The repo already exposes the core surfaces an iPhone app needs:

- `crates/ship-service/src/lib.rs` defines the `Ship` service methods for session listing, session detail, steering, review actions, permission resolution, event subscription, global subscription, and audio transcription.
- `frontend/src/hooks/useSession.ts` and `frontend/src/hooks/useSessionState.ts` already encode the intended hydration and reconnect flow.
- `frontend/src/state/sessionReducer.ts` already defines the materialized session view derived from events rather than ad hoc UI state.
- `frontend/src/api/client.ts` shows the single-WebSocket roam client pattern.
- `frontend/src/hooks/useTranscription.ts`, `frontend/src/components/UnifiedComposer.tsx`, `crates/ship-server/src/transcriber.rs`, and `crates/ship-server/src/ship_impl.rs` show that speech-to-text is already a backend capability.

That means the main iOS work is native transport, reducer/state, block rendering, composer UX, and mobile-specific polish.

## MVP Feature Set

- Connect to a Ship server over WebSocket.
- Show the global session list and basic session metadata.
- Open a session with the same two-phase hydration model as web.
- Render the unified feed for captain and mate blocks.
- Surface task status, agent state, plan state, and pending human review or permission items.
- Send text to the captain and steer the mate.
- Accept, cancel, reply to human, resolve permission, retry agent, and stop agents.
- Optional but reasonable in MVP: hold-to-talk or tap-to-talk transcription using the existing `transcribe_audio` stream.

## Phased Plan

### Day 1

- Prove the transport and protocol shape with a thin client.
- Implement `list_sessions`, `get_session`, `subscribe_events`, and basic reducer-driven rendering.
- Show a simple unified feed and allow sending a text prompt.

### Days 2-3

- Flesh out session detail screens, reconnect handling, replay completion, and gap recovery.
- Add controls for steer, accept, cancel, retry, stop, reply-to-human, and permission resolution.
- Add plan and agent-state presentation.

### Week 1

- Tighten the composer, block rendering, and review flows for real internal use.
- Add audio transcription UI if it did not land on day 1.
- Add test fixtures for representative event streams and reducer behavior.
- Make the app stable enough for daily dogfooding.

### Later Polish

- Better notifications and attention management.
- More refined block rendering for long tool output and diffs.
- Better voice UX, interruption handling, and transcript editing.
- Optional iPad and macOS/Catalyst improvements once the iPhone interaction model is solid.
- Only consider push notifications after the internal app proves useful enough to justify the operational surface.

## Iteration Strategy

Use agent-driven, fixture-heavy iteration rather than building only against a live backend.

- SwiftUI previews for individual feed blocks, agent-state cards, permission rows, and composer states.
- Fixture-driven development for session replay: serialize representative `SessionDetail` values plus `SubscribeMessage` streams and drive the reducer with them.
- Screenshot or snapshot workflow for key states: loading, replaying, live session, permission pending, review pending, error, and audio recording.
- XCUITest with stable accessibility identifiers for core flows: open session, replay completes, send captain prompt, resolve permission, accept task, retry agent.
- Use the simulator for integration passes, previews for fast UI iteration, and optionally a Catalyst target if desktop debugging materially speeds up iteration.

## Risks And Hard Parts

- Replay and reconnect correctness: the app has to respect Ship's sequence-based event model and rebuild from replay rather than merge into stale local state.
- Block and event rendering: Ship's UI is a structured feed, not a terminal. The reducer and block renderers are the real product surface.
- Audio-session edge cases: microphone permission, interruptions, route changes, backgrounding, and the difference between transcription transport success and good UX.
- Notifications and push scope: local in-app attention patterns are cheap; remote push is a separate product and operational decision.

## Rough Size Estimate

For one engineer who is comfortable with SwiftUI and the existing Ship model:

- Thin prototype: about 1 day.
- Internal MVP: about 3-5 working days.
- Polished internal app: about 2-4 weeks, depending mostly on audio, notifications, renderer quality, and QA.

## Recommendation

Build the thinnest useful client first. The practical next step is a SwiftUI prototype that proves four things quickly: roam WebSocket connectivity, `get_session` plus `subscribe_events` hydration, reducer-driven unified feed rendering, and basic prompt or steer submission. If those feel solid, the rest is mostly iterative product work rather than fundamental platform risk.

## Grounding In Current Repo

- `README.md`: architecture overview and the backend/frontend split.
- `crates/ship-service/src/lib.rs`: `Ship` service trait, including `get_session`, `subscribe_events`, `subscribe_global_events`, action RPCs, and `transcribe_audio`.
- `frontend/src/api/client.ts`: current roam WebSocket client setup.
- `frontend/src/hooks/useSession.ts`: structural session hydration.
- `frontend/src/hooks/useSessionState.ts`: replay, reconnect, and subscription lifecycle.
- `frontend/src/state/sessionReducer.ts`: derived session view state and event reducer.
- `frontend/src/components/UnifiedComposer.tsx`: current composer responsibilities and action mapping.
- `frontend/src/hooks/useTranscription.ts`: frontend audio capture and streaming transcription usage.
- `crates/ship-server/src/transcriber.rs`: backend streaming speech segmentation and transcription.
- `crates/ship-server/src/ship_impl.rs`: server-side `transcribe_audio` wiring.
