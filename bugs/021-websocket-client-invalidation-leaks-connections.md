# Websocket client invalidation leaks connections

## Symptom

The Ship server eventually starts logging:

```text
ERROR axum::serve::listener: accept error: Too many open files (os error 24)
```

When this happens, the server process is dominated by accepted TCP connections to `localhost:9140` rather than regular files or pipes.

## Expected behavior

When the frontend reconnects or invalidates a websocket client, the old websocket connection should be closed explicitly. A single browser tab should not leave behind a growing pile of established websocket connections.

## Owner

`fullstack`

## Evidence

- `lsof -p <ship-pid>` shows the process holding a very large number of `TCP localhost:9140->localhost:* (ESTABLISHED)` descriptors.
- `frontend/src/api/client.ts` creates websocket clients with `connectWs(...)` but `invalidateShipClient(...)` only nulls the cached promise.
- `frontend/src/hooks/useSessionState.ts` invalidates and recreates clients on reconnect/failure paths, but there is no explicit close/dispose of the previous websocket.

## Suspected root cause

The frontend websocket client cache has an invalidation path but no real shutdown path. When reconnect logic forces a new client, the previous websocket remains open, so the server accumulates accepted sockets until it hits the process file descriptor limit.

## Spec impact

- `r[event.client.connection-lifecycle]`
- `r[event.subscribe]`

## Next action

- Make the frontend websocket client own a real closeable transport/connection handle.
- On invalidation, explicitly close the old websocket before dropping it.
- Add a regression test or diagnostic that proves reconnect does not grow live websocket count indefinitely.
