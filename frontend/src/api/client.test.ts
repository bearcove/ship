import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const helloExchangeInitiator = vi.fn();
const defaultHello = vi.fn(() => ({ tag: "hello" }));
const ShipClient = vi.fn(function ShipClient(this: { caller: unknown }, caller: unknown) {
  this.caller = caller;
});

class FakeWebSocket {
  static instances: FakeWebSocket[] = [];

  readonly url: string;
  binaryType = "";
  private listeners = new Map<string, Set<(event: any) => void>>();

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
    queueMicrotask(() => {
      this.dispatch("open", {});
    });
  }

  addEventListener(type: string, listener: (event: any) => void) {
    let listeners = this.listeners.get(type);
    if (!listeners) {
      listeners = new Set();
      this.listeners.set(type, listeners);
    }
    listeners.add(listener);
  }

  removeEventListener(type: string, listener: (event: any) => void) {
    this.listeners.get(type)?.delete(listener);
  }

  dispatch(type: string, event: any) {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }
}

vi.mock("@bearcove/roam-ws", () => ({
  WsTransport: class WsTransport {
    constructor(readonly socket: FakeWebSocket) {}
  },
}));

vi.mock("@bearcove/roam-core", () => ({
  defaultHello,
  helloExchangeInitiator,
}));

vi.mock("../generated/ship", () => ({
  ShipClient,
}));

describe("client lifecycle", () => {
  beforeEach(() => {
    vi.stubGlobal("WebSocket", FakeWebSocket);
    FakeWebSocket.instances = [];
  });

  afterEach(async () => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
    vi.resetModules();
    vi.clearAllMocks();
  });

  it("closes the active websocket client when invalidated", async () => {
    const close = vi.fn();
    const asCaller = vi.fn(() => ({ caller: "one" }));
    helloExchangeInitiator.mockResolvedValue({ getIo: () => ({ close }), asCaller });

    const mod = await import("./client");
    await mod.getShipClient();
    mod.invalidateShipClient("test");

    expect(close).toHaveBeenCalledTimes(1);
  });

  it("closes the previous websocket client when forceNew is requested", async () => {
    const close1 = vi.fn();
    const close2 = vi.fn();

    helloExchangeInitiator
      .mockResolvedValueOnce({
        getIo: () => ({ close: close1 }),
        asCaller: () => ({ caller: "one" }),
      })
      .mockResolvedValueOnce({
        getIo: () => ({ close: close2 }),
        asCaller: () => ({ caller: "two" }),
      });

    const mod = await import("./client");
    await mod.getShipClient();
    await mod.getShipClient({ forceNew: true });

    expect(close1).toHaveBeenCalledTimes(1);
    expect(close2).not.toHaveBeenCalled();
  });

  it("opens a fresh websocket after the transport closes unexpectedly", async () => {
    const close1 = vi.fn();
    const close2 = vi.fn();

    helloExchangeInitiator
      .mockResolvedValueOnce({
        getIo: () => ({ close: close1 }),
        asCaller: () => ({ caller: "one" }),
      })
      .mockResolvedValueOnce({
        getIo: () => ({ close: close2 }),
        asCaller: () => ({ caller: "two" }),
      });

    const mod = await import("./client");
    const firstClient = await mod.getShipClient();
    expect(FakeWebSocket.instances).toHaveLength(1);

    FakeWebSocket.instances[0]!.dispatch("close", {
      code: 1006,
      reason: "transport lost",
      wasClean: false,
    });

    const secondClient = await mod.getShipClient();

    expect(FakeWebSocket.instances).toHaveLength(2);
    expect(secondClient).not.toBe(firstClient);
    expect(close1).not.toHaveBeenCalled();
    expect(close2).not.toHaveBeenCalled();
  });
});
