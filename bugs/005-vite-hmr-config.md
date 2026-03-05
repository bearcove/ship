# 005: Vite dev server HMR configuration is wrong

Status: open
Owner: backend

## Symptom

Vite HMR websocket setup fails in development, and startup behavior clears the screen when Ship starts Vite.

## Expected Behavior

- Starting Ship should not clear the terminal screen.
- Vite HMR should use the correct websocket configuration, with HMR listening on a separate websocket port/path as needed.

## Evidence

Browser console excerpt:

```text
[vite] failed to connect to websocket.
your current setup:
(browser) [::]:9140/ <--[HTTP]--> 127.0.0.1:5173/ (server)
(browser) [::]:9140/ <--[WebSocket (failing)]--> 127.0.0.1:5173/ (server)
```

## Suspected Root Cause

The backend-spawned Vite config does not set the HMR websocket host/port correctly for Ship's proxy topology, and Vite startup flags/defaults are still clearing the screen.

## Spec Impact

This is development-environment behavior, but it materially harms local use and debugging.

## Next Action

- Adjust the backend-managed Vite launch/config so the screen is not cleared on startup.
- Configure HMR websocket settings explicitly for Ship's proxied dev setup.
- Verify browser console is clean and HMR reconnects correctly.
