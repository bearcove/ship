# 017: Create session blocks on agent startup and ACP handshake

Status: open
Owner: fullstack

## Symptom

Submitting the new-session dialog does not immediately navigate to the session page. Instead, session creation blocks on backend startup work such as:
- launching ACP agents
- session/transport setup
- initial handshake and related initialization

This keeps the dialog in the critical path longer than necessary.

## Expected Behavior

`Cmd+Enter` or clicking `Create session` should create the session record immediately and navigate to the session page right away. Agent startup, ACP handshake, and similar work should continue in the background, with progress and failures streamed to the session view.

## Evidence

User report from live use of the new-session flow.

## Suspected Root Cause

Session creation currently bundles durable session creation with downstream runtime startup, instead of treating runtime startup as asynchronous session/task progress.

## Spec Impact

Session creation flow, task/session startup lifecycle, and how startup progress is surfaced to the client.

## Next Action

- split durable session creation from background agent/session startup
- navigate to the session page immediately after creation succeeds
- stream startup progress and failures to the session view instead of blocking the dialog
