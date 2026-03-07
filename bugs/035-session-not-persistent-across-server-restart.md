# 035: Sessions and agent context lost on server restart

Status: open
Owner: backend

## Symptom

Restarting the ship server loses all active session state. Agent conversations are gone. The session list may be recoverable from disk but the ACP sessions are not.

## Expected behavior

Sessions survive a server restart. The preferred approach is to use the ACP experimental session persistence API so the agent's conversation context is preserved across process restarts without manual serialization.

## Options considered

1. Serialize session context to disk and re-inject on restart (fragile, lossy)
2. ACP proxy process that outlives the ship server (operational complexity)
3. ACP session persistence API (preferred — let ACP handle it)

## Next action

- Investigate ACP session persistence API availability and stability
- Implement option 3 if API is usable
