import { channel } from "@bearcove/roam-core";
import type { GlobalEvent, ProjectInfo, SessionSummary } from "../generated/ship";
import { getShipClient, onClientReady } from "../api/client";

// --- Session list state ---
let cachedSessions: SessionSummary[] = [];
type SessionListListener = (sessions: SessionSummary[]) => void;
const sessionListListeners = new Set<SessionListListener>();

export function onSessionListChanged(cb: SessionListListener): () => void {
  sessionListListeners.add(cb);
  cb(cachedSessions);
  return () => sessionListListeners.delete(cb);
}

// --- Project list state ---
let cachedProjects: ProjectInfo[] = [];
type ProjectListListener = (projects: ProjectInfo[]) => void;
const projectListListeners = new Set<ProjectListListener>();

export function onProjectListChanged(cb: ProjectListListener): () => void {
  projectListListeners.add(cb);
  cb(cachedProjects);
  return () => projectListListeners.delete(cb);
}

// --- Subscription lifecycle ---
let subscriptionActive = false;
let retryTimer: ReturnType<typeof setTimeout> | null = null;

function handleGlobalEvent(event: GlobalEvent) {
  if (event.tag === "SessionListChanged") {
    cachedSessions = event.sessions;
    for (const cb of sessionListListeners) cb(cachedSessions);
  } else if (event.tag === "ProjectListChanged") {
    cachedProjects = event.projects;
    for (const cb of projectListListeners) cb(cachedProjects);
  }
}

async function startGlobalSubscription() {
  if (subscriptionActive) return;
  subscriptionActive = true;
  try {
    const client = await getShipClient();
    const [tx, rx] = channel<GlobalEvent>();
    await client.subscribeGlobalEvents(tx);

    while (true) {
      const msg = await rx.recv();
      if (msg === null) break;
      handleGlobalEvent(msg);
    }
  } catch {
  } finally {
    subscriptionActive = false;
    if (retryTimer !== null) clearTimeout(retryTimer);
    retryTimer = setTimeout(() => {
      retryTimer = null;
      void startGlobalSubscription();
    }, 3000);
  }
}

// Start on load
void startGlobalSubscription();

// Restart on new connection
onClientReady(() => {
  if (!subscriptionActive) {
    void startGlobalSubscription();
  }
});
