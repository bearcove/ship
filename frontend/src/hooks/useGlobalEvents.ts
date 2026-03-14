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

// --- Desktop notifications for is_read transitions ---
const MAX_INDIVIDUAL_NOTIFICATIONS = 3;

function notifyUnreadSessions(newSessions: SessionSummary[]) {
  if (document.hasFocus()) return;
  if (!("Notification" in window) || Notification.permission !== "granted") return;

  const oldById = new Map(cachedSessions.map((s) => [s.id, s]));
  const flipped: SessionSummary[] = [];
  for (const s of newSessions) {
    if (s.is_read) continue;
    const old = oldById.get(s.id);
    if (old && !old.is_read) continue; // was already unread
    flipped.push(s);
  }

  if (flipped.length === 0) return;

  if (flipped.length > MAX_INDIVIDUAL_NOTIFICATIONS) {
    const n = new Notification("Ship", {
      body: `${flipped.length} sessions need your attention`,
      tag: "ship-batch",
    });
    n.onclick = () => {
      window.focus();
      window.location.href = "/";
    };
    return;
  }

  for (const session of flipped) {
    const title = session.title ?? "Untitled session";
    const isWaiting = session.task_status?.tag === "WaitingForHuman";
    const body = isWaiting
      ? "Captain has a question"
      : "Task complete \u2014 ready for new work";

    const n = new Notification(title, {
      body,
      tag: `ship-session-${session.id}`,
    });
    n.onclick = () => {
      window.focus();
      window.location.href = `/sessions/${session.slug}`;
    };
  }
}

function handleGlobalEvent(event: GlobalEvent) {
  if (event.tag === "SessionListChanged") {
    notifyUnreadSessions(event.sessions);
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
