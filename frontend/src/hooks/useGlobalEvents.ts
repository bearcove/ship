import { channel } from "@bearcove/roam-core";
import type { ActivityEntry, GlobalEvent, ProjectInfo, SessionSummary } from "../generated/ship";
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

// --- Activity entries state ---
let cachedActivityEntries: ActivityEntry[] = [];
type ActivityListener = (entries: ActivityEntry[]) => void;
const activityListeners = new Set<ActivityListener>();

export function onActivityChanged(cb: ActivityListener): () => void {
  activityListeners.add(cb);
  cb(cachedActivityEntries);
  return () => activityListeners.delete(cb);
}

// --- Subscription lifecycle ---
let subscriptionActive = false;
let retryTimer: ReturnType<typeof setTimeout> | null = null;

const ACTIVITY_MAX_ENTRIES = 200;

function handleGlobalEvent(event: GlobalEvent) {
  if (event.tag === "SessionListChanged") {
    cachedSessions = event.sessions;
    for (const cb of sessionListListeners) cb(cachedSessions);
  } else if (event.tag === "ProjectListChanged") {
    cachedProjects = event.projects;
    for (const cb of projectListListeners) cb(cachedProjects);
  } else if (event.tag === "Activity") {
    cachedActivityEntries = [...cachedActivityEntries, event.entry];
    if (cachedActivityEntries.length > ACTIVITY_MAX_ENTRIES) {
      cachedActivityEntries = cachedActivityEntries.slice(
        cachedActivityEntries.length - ACTIVITY_MAX_ENTRIES,
      );
    }
    for (const cb of activityListeners) cb(cachedActivityEntries);
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
