# Frontend error handling tears down WebSocket on individual RPC failures

## Problem

The frontend treats every individual RPC or subscription error as "the connection is broken" and tears down the entire WebSocket. This is wrong — Roam already multiplexes over a single connection with per-request IDs and per-channel stream IDs. Multiple concurrent RPCs and subscriptions coexist without interfering.

The coupling is entirely in the frontend's error handling — it conflates "this one call failed" with "the whole transport is dead."

## Fix

1. **Stop calling `invalidateShipClient` on individual failures.** If `listSessions` throws, that's a failed RPC — retry the RPC, don't nuke the WebSocket. If a subscription channel closes, resubscribe on the same connection.

2. **Only tear down the WebSocket when the WebSocket itself dies** — i.e., the transport-level `close` or `error` event fires. That's when you know the connection is actually gone.

3. **Remove `forceNew` from retry paths.** `useSessionState.ts:229` should just call `getShipClient()` (no `forceNew`) and resubscribe.

## Why not virtual connections?

No real downside to the current single-connection model. Roam already isolates RPCs and channels from each other at the wire level. Virtual connections would add complexity for isolation that isn't needed.
