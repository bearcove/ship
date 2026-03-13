# Native iPhone Client Plan

## Executive Summary

Building a Ship iPhone client is viable because Ship already owns the hard domain logic. The backend already defines the typed RPC surface, session event stream with replay, reducer-friendly client model, and the server-side transcription pipeline. The iPhone app is therefore mostly a native client over existing Ship RPCs, event streams, and transcription, not a new backend project.

The right sequencing is to prove a thin client first. A rough but real day-1 prototype is realistic: connect to Ship, list sessions, open a session, replay the feed, and send text. That is enough to prove the transport, hydration model, and basic interaction loop. A polished internal app is a longer effort mainly because of replay and reconnect correctness, structured renderer quality, notifications, audio-session edge cases, and QA. This should be scoped as an internal app first, not App Store-grade by default.

## Recommended Architecture

The app should be SwiftUI-first. Use UIKit only where it is the pragmatic implementation detail, especially for a wrapped `UITextView` composer if SwiftUI's text editing limits get in the way.

The client architecture should stay close to the current web shape:

- One transport layer for the Ship roam WebSocket connection, analogous to `frontend/src/api/client.ts`.
- A thin typed client for the `Ship` service methods in `crates/ship-service/src/lib.rs`.
- A session store that follows the same two-phase flow as the web client: `get_session` for structural state, then `subscribe_events` for replay plus live updates.
- A reducer-driven view model, mirroring the responsibilities currently encoded in `frontend/src/state/sessionReducer.ts`.
- A SwiftUI shell around a UIKit-backed text editor for the composer if needed.
- Optional voice input UI layered over the existing `transcribe_audio` RPC and server-side transcription path.

## What The App Actually Renders

The app should not be framed as generic rich-text chat. Ship's UI is a structured event and block feed. The iPhone client should render typed block views, not one giant attributed text blob.

The rendering model should follow the existing web event pipeline:

- Session chrome and structural metadata come from `get_session`.
- Live content comes from `subscribe_events`, including replay and reconnect behavior.
- The reducer materializes view state from ordered events rather than relying on ad hoc screen-local state.
- Rich rendering should come from typed block views such as text, tool call, plan update, permission, and image blocks.
- The composer is a control surface over Ship actions, not just a text field: send to captain, steer mate, attach images, and optionally inject transcript text.

In practice, this means the core UI work is block rendering, session-state reduction, and a mobile-appropriate composer shell around those existing backend surfaces.

## Prototype-To-Polish Phases

### Day 1

- Prove the thin client end to end.
- Connect to Ship, list sessions, open a session, replay the current feed, and send text.
- Render a rough but real structured feed and confirm the basic hydration model works on device or simulator.

### Days 2-3

- Add the key control actions: steer, accept, cancel, reply-to-human, resolve permission, retry agent, stop agents.
- Tighten replay completion, reconnect handling, and gap recovery.
- Replace rough feed rows with clearer typed block views.

### Week 1

- Make it usable as an internal app.
- Improve the composer, including a UIKit-backed editor if that is the pragmatic path.
- Add plan, permission, and review rendering that matches Ship's structured model.
- Layer in transcription UI if it did not land on day 1.

### Later Polish

- Improve renderer quality for long tool output, diffs, and dense plan or permission states.
- Add better notification behavior and attention management.
- Handle audio-session interruptions, routing changes, and backgrounding cleanly.
- Spend real time on QA, because this is where a rough prototype turns into a dependable internal tool.

## Iteration Strategy For Agent-Driven Development

Without extra tooling, native UI work is a poor agent fit. The workable loop is artifact-driven, not browser-style. Do not assume native iteration is equivalent to browser HMR.

The practical loop is:

- SwiftUI previews for fast work on individual block views, agent-state rows, composer states, and loading or replay states.
- Fixed fixtures for `SessionDetail` plus representative `SubscribeMessage` streams so reducer and renderer work can be repeated deterministically.
- Screenshot or snapshot generation for key states so an agent can see concrete artifacts instead of guessing from prose.
- XCUITest and UI automation via stable accessibility identifiers for flows like open session, replay completes, send prompt, resolve permission, and accept task.
- Simulator runs for integration validation; previews and fixtures for rapid iteration.
- Optional Catalyst or macOS support only if it materially improves debugging speed.

Native automation does exist, but it is accessibility-tree and UI-test driven rather than DOM-driven. That difference matters: the plan should optimize for artifacts an agent can inspect, not for a browser-like live-edit loop that iOS does not provide.

## Risks / Hard Parts

- Replay and reconnect correctness. The app has to respect Ship's sequence-based event model and rebuild from replay rather than merging into stale state.
- Renderer quality. The product surface is a structured block feed, so typed rendering quality matters more than generic chat polish.
- Notifications. Local attention management is straightforward; anything beyond that quickly becomes product and operational scope.
- Audio-session edge cases. Microphone permission, interruptions, output route changes, backgrounding, and transcript UX are all real complexity.
- QA. Mobile lifecycle bugs, reconnect timing issues, and renderer edge cases are what separate a convincing prototype from a dependable internal app.

## Grounding In The Current Repo

- `README.md`: high-level architecture and the existing unified feed and composer model.
- `crates/ship-service/src/lib.rs`: the `Ship` service trait, including `list_sessions`, `get_session`, `subscribe_events`, action RPCs, and `transcribe_audio`.
- `frontend/src/api/client.ts`: the current single-WebSocket roam transport pattern.
- `frontend/src/hooks/useSession.ts`: structural hydration via `get_session`.
- `frontend/src/hooks/useSessionState.ts`: subscription lifecycle, replay buffering, replay completion, reconnect handling, and live event processing.
- `frontend/src/state/sessionReducer.ts`: reducer-driven session view state and structured block application.
- `frontend/src/components/UnifiedComposer.tsx`: current composer responsibilities and action mapping.
- `frontend/src/hooks/useTranscription.ts`: frontend audio capture and streaming transcription usage.
- `crates/ship-server/src/transcriber.rs`: server-side segmentation and transcription.
- `crates/ship-server/src/ship_impl.rs`: server wiring for `transcribe_audio`.
