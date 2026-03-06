// r[backend.rpc]
import { defaultHello, helloExchangeInitiator } from "@bearcove/roam-core";
import { connectWs } from "@bearcove/roam-ws";
import { ShipClient } from "../generated/ship";

export type { ShipClient } from "../generated/ship";

type CloseableConnection = {
  getIo(): { close(): void };
  asCaller(): ConstructorParameters<typeof ShipClient>[0];
};

type ShipClientHandle = {
  attempt: number;
  client: ShipClient;
  connection: CloseableConnection;
};

let clientPromise: Promise<ShipClientHandle> | null = null;
let activeHandle: ShipClientHandle | null = null;
let connectionAttempt = 0;
let clientGeneration = 0;

function log(level: "info" | "warn", message: string, details?: Record<string, unknown>) {
  const method = level === "warn" ? console.warn : console.info;
  method(`[ship/ws] ${message}`, details ?? {});
}

function closeActiveClient(reason: string) {
  if (!activeHandle) return;
  log("info", "closing websocket client", {
    attempt: activeHandle.attempt,
    reason,
  });
  activeHandle.connection.getIo().close();
  activeHandle = null;
}

async function createShipClient(generation: number): Promise<ShipClientHandle> {
  const attempt = ++connectionAttempt;
  log("info", "opening websocket client", { attempt, url: "ws://localhost:9140/ws" });
  const transport = await connectWs("ws://localhost:9140/ws");
  const connection = await helloExchangeInitiator(transport, defaultHello(), {
    keepalive: { pingIntervalMs: 5000, pongTimeoutMs: 10000 },
  });
  if (generation !== clientGeneration) {
    log("warn", "closing stale websocket client", {
      attempt,
      generation,
      current: clientGeneration,
    });
    connection.getIo().close();
    throw new Error("stale websocket client");
  }
  log("info", "websocket client ready", { attempt });
  const handle = {
    attempt,
    client: new ShipClient(connection.asCaller()),
    connection,
  };
  activeHandle = handle;
  return handle;
}

export function getShipClient(options?: { forceNew?: boolean }): Promise<ShipClient> {
  if (options?.forceNew || clientPromise === null) {
    clientGeneration += 1;
    if (options?.forceNew) {
      closeActiveClient("forceNew client requested");
    }
    clientPromise = createShipClient(clientGeneration);
  }
  return clientPromise.then((handle) => handle.client);
}

export function invalidateShipClient(reason: string) {
  log("warn", "invalidating websocket client", { reason });
  clientGeneration += 1;
  closeActiveClient(reason);
  clientPromise = null;
}
