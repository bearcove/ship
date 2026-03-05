// r[backend.rpc]
import { defaultHello, helloExchangeInitiator } from "@bearcove/roam-core";
import { connectWs } from "@bearcove/roam-ws";
import { ShipClient } from "../generated/ship";
export type { ShipClient } from "../generated/ship";

// Connect with keepalive so the message pump exits promptly on connection drop
// rather than spinning for 30 seconds until the request timeout fires.
export const shipClient: Promise<ShipClient> = (async () => {
  const transport = await connectWs("ws://localhost:9140/ws");
  const connection = await helloExchangeInitiator(transport, defaultHello(), {
    keepalive: { pingIntervalMs: 5000, pongTimeoutMs: 10000 },
  });
  return new ShipClient(connection.asCaller());
})();
