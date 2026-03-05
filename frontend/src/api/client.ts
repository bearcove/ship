// r[backend.rpc]
import { connectShip } from "../generated/ship";
export type { ShipClient } from "../generated/ship";

export const shipClient = connectShip("ws://localhost:9140/ws");
