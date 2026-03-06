// r[backend.rpc]
import { defaultHello, helloExchangeInitiator } from "@bearcove/roam-core";
import { connectWs } from "@bearcove/roam-ws";
import { ShipClient } from "../generated/ship";

export type { ShipClient } from "../generated/ship";

let clientPromise: Promise<ShipClient> | null = null;
let connectionAttempt = 0;

function log(level: "info" | "warn", message: string, details?: Record<string, unknown>) {
  const method = level === "warn" ? console.warn : console.info;
  method(`[ship/ws] ${message}`, details ?? {});
}

async function createShipClient(): Promise<ShipClient> {
  const attempt = ++connectionAttempt;
  log("info", "opening websocket client", { attempt, url: "ws://localhost:9140/ws" });
  const transport = await connectWs("ws://localhost:9140/ws");
  const connection = await helloExchangeInitiator(transport, defaultHello(), {
    keepalive: { pingIntervalMs: 5000, pongTimeoutMs: 10000 },
  });
  log("info", "websocket client ready", { attempt });
  return new ShipClient(connection.asCaller());
}

export function getShipClient(options?: { forceNew?: boolean }): Promise<ShipClient> {
  if (options?.forceNew || clientPromise === null) {
    clientPromise = createShipClient();
  }
  return clientPromise;
}

export function invalidateShipClient(reason: string) {
  log("warn", "invalidating websocket client", { reason });
  clientPromise = null;
}
