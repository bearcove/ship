// r[backend.rpc]
import { useEffect, useState } from "react";
import { defaultHello, helloExchangeInitiator } from "@bearcove/roam-core";
import { WsTransport } from "@bearcove/roam-ws";
import { ShipClient } from "../generated/ship";

export type { ShipClient } from "../generated/ship";

declare global {
  interface Window {
    __SHIP_SERVED__?: boolean;
  }
}

export type ClientLogEntry = {
  level: "info" | "warn";
  message: string;
  details: Record<string, unknown>;
  ts: number;
};

const MAX_LOG_ENTRIES = 200;
const logBuffer: ClientLogEntry[] = [];
const logListeners = new Set<() => void>();

// Capture [roam-ws ...] console.info messages into our log buffer
const origConsoleInfo = console.info;
console.info = (...args: unknown[]) => {
  origConsoleInfo.apply(console, args);
  if (typeof args[0] === "string" && args[0].startsWith("[roam-ws")) {
    const entry: ClientLogEntry = {
      level: "info",
      message: String(args[0]),
      details: args[1] && typeof args[1] === "object" ? (args[1] as Record<string, unknown>) : {},
      ts: Date.now(),
    };
    if (logBuffer.length >= MAX_LOG_ENTRIES) logBuffer.shift();
    logBuffer.push(entry);
    notifyLogListeners();
  }
};

function notifyLogListeners() {
  for (const cb of logListeners) cb();
}

export function useClientLogs(): ClientLogEntry[] {
  const [entries, setEntries] = useState<ClientLogEntry[]>(() => [...logBuffer]);
  useEffect(() => {
    function update() {
      setEntries([...logBuffer]);
    }
    logListeners.add(update);
    update(); // sync anything that fired before we subscribed
    return () => {
      logListeners.delete(update);
    };
  }, []);
  return entries;
}

// --- Connection state ---

export type ConnectionState = "initial-connecting" | "connected" | "reconnecting" | "wrong-port";

let connectionState: ConnectionState = "initial-connecting";
const connectionStateListeners = new Set<(state: ConnectionState) => void>();

function setConnectionState(state: ConnectionState) {
  connectionState = state;
  for (const cb of connectionStateListeners) cb(state);
}

/** Subscribe to connection state changes. Returns an unsubscribe function. */
export function onConnectionStateChanged(cb: (state: ConnectionState) => void): () => void {
  connectionStateListeners.add(cb);
  return () => connectionStateListeners.delete(cb);
}

/** Get the current connection state. */
export function getConnectionState(): ConnectionState {
  return connectionState;
}

// --- WebSocket internals ---

type OpenWebSocket = {
  socket: WebSocket;
  transport: WsTransport;
};

function connectWsOpen(url: string): Promise<OpenWebSocket> {
  return new Promise((resolve, reject) => {
    log("info", "opening WebSocket", { url });
    const ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";

    let settled = false;

    ws.addEventListener("open", () => {
      settled = true;
      resolve({ socket: ws, transport: new WsTransport(ws) });
    });

    ws.addEventListener("error", () => {
      if (!settled) {
        settled = true;
        reject(new Error(`WebSocket connection failed: ${url}`));
      }
    });

    ws.addEventListener("close", (ev) => {
      if (!settled) {
        settled = true;
        reject(new Error(`WebSocket closed before open: ${ev.code} ${ev.reason}`));
      }
    });
  });
}

type CloseableConnection = {
  getIo(): { close(): void };
  asCaller(): ConstructorParameters<typeof ShipClient>[0];
};

type ShipClientHandle = {
  attempt: number;
  generation: number;
  client: ShipClient;
  connection: CloseableConnection;
  socket: WebSocket;
  closedByClient: boolean;
};

let clientPromise: Promise<ShipClientHandle> | null = null;
let activeHandle: ShipClientHandle | null = null;
let connectionAttempt = 0;
let clientGeneration = 0;
let retryTimer: ReturnType<typeof setTimeout> | null = null;

const clientReadyListeners = new Set<() => void>();

/** Subscribe to be notified whenever a new WebSocket connection is established. */
export function onClientReady(cb: () => void): () => void {
  clientReadyListeners.add(cb);
  return () => clientReadyListeners.delete(cb);
}

function log(level: "info" | "warn", message: string, details?: Record<string, unknown>) {
  const method = level === "warn" ? console.warn : console.info;
  method(`[ship/ws] ${message}`, details ?? {});
  const entry: ClientLogEntry = { level, message, details: details ?? {}, ts: Date.now() };
  if (logBuffer.length >= MAX_LOG_ENTRIES) logBuffer.shift();
  logBuffer.push(entry);
  notifyLogListeners();
}

function closeActiveClient(reason: string) {
  if (!activeHandle) return;
  log("info", "closing websocket client", {
    attempt: activeHandle.attempt,
    reason,
  });
  activeHandle.closedByClient = true;
  activeHandle.connection.getIo().close();
  activeHandle = null;
}

function scheduleRetry() {
  // Don't retry if user is on the wrong port — retrying won't help.
  if (connectionState === "wrong-port") return;
  if (retryTimer !== null) return;
  retryTimer = setTimeout(() => {
    retryTimer = null;
    void getShipClient();
  }, 3000);
}

function handleTransportDeath(
  handle: ShipClientHandle,
  kind: "close" | "error",
  details: Record<string, unknown>,
) {
  if (handle.closedByClient) return;
  if (activeHandle !== handle) return;
  if (handle.generation !== clientGeneration) return;
  log("warn", "websocket transport died", {
    attempt: handle.attempt,
    kind,
    ...details,
  });
  clientGeneration += 1;
  activeHandle = null;
  clientPromise = null;
  setConnectionState("reconnecting");
  scheduleRetry();
}

async function createShipClient(generation: number): Promise<ShipClientHandle> {
  const attempt = ++connectionAttempt;
  if (!window.__SHIP_SERVED__) {
    setConnectionState("wrong-port");
    throw new Error(
      "window.__SHIP_SERVED__ is not set — open Ship via its server port, not Vite directly",
    );
  }
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const wsUrl = `${protocol}//${window.location.host}/ws`;
  log("info", "opening websocket client", { attempt, url: wsUrl });
  const { socket, transport } = await connectWsOpen(wsUrl);
  const connection = await helloExchangeInitiator(transport, defaultHello(), {
    keepalive: { pingIntervalMs: 5000, pongTimeoutMs: 10000 },
  });
  const handle = {
    attempt,
    generation,
    client: new ShipClient(connection.asCaller()),
    connection,
    socket,
    closedByClient: false,
  };
  socket.addEventListener("close", (event) => {
    handleTransportDeath(handle, "close", {
      code: event.code,
      reason: event.reason,
      wasClean: event.wasClean,
    });
  });
  socket.addEventListener("error", () => {
    handleTransportDeath(handle, "error", {});
  });
  if (generation !== clientGeneration) {
    log("warn", "closing stale websocket client", {
      attempt,
      generation,
      current: clientGeneration,
    });
    handle.closedByClient = true;
    connection.getIo().close();
    throw new Error("stale websocket client");
  }
  log("info", "websocket client ready", { attempt });
  activeHandle = handle;
  setConnectionState("connected");
  for (const cb of clientReadyListeners) cb();
  return handle;
}

export function getShipClient(options?: { forceNew?: boolean }): Promise<ShipClient> {
  if (options?.forceNew || clientPromise === null) {
    clientGeneration += 1;
    if (options?.forceNew) {
      closeActiveClient("forceNew client requested");
    }
    const p = createShipClient(clientGeneration);
    clientPromise = p;
    p.catch(() => {
      if (clientPromise === p) {
        clientPromise = null;
        scheduleRetry();
      }
    });
  }
  return clientPromise.then((handle) => handle.client);
}

export function invalidateShipClient(reason: string) {
  log("warn", "invalidating websocket client", { reason });
  clientGeneration += 1;
  closeActiveClient(reason);
  clientPromise = null;
  scheduleRetry();
}
