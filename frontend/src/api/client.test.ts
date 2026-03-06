import { afterEach, describe, expect, it, vi } from "vitest";

const connectWs = vi.fn();
const helloExchangeInitiator = vi.fn();
const defaultHello = vi.fn(() => ({ tag: "hello" }));
const ShipClient = vi.fn(function ShipClient(this: { caller: unknown }, caller: unknown) {
  this.caller = caller;
});

vi.mock("@bearcove/roam-ws", () => ({
  connectWs,
}));

vi.mock("@bearcove/roam-core", () => ({
  defaultHello,
  helloExchangeInitiator,
}));

vi.mock("../generated/ship", () => ({
  ShipClient,
}));

describe("client lifecycle", () => {
  afterEach(async () => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it("closes the active websocket client when invalidated", async () => {
    const close = vi.fn();
    const asCaller = vi.fn(() => ({ caller: "one" }));
    connectWs.mockResolvedValue({});
    helloExchangeInitiator.mockResolvedValue({ getIo: () => ({ close }), asCaller });

    const mod = await import("./client");
    await mod.getShipClient();
    mod.invalidateShipClient("test");

    expect(close).toHaveBeenCalledTimes(1);
  });

  it("closes the previous websocket client when forceNew is requested", async () => {
    const close1 = vi.fn();
    const close2 = vi.fn();

    connectWs.mockResolvedValueOnce({}).mockResolvedValueOnce({});
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

  it("closes every superseded websocket across reconnect cycles", async () => {
    const closes = [vi.fn(), vi.fn(), vi.fn()];

    connectWs.mockResolvedValue({});
    helloExchangeInitiator
      .mockResolvedValueOnce({
        getIo: () => ({ close: closes[0] }),
        asCaller: () => ({ caller: "one" }),
      })
      .mockResolvedValueOnce({
        getIo: () => ({ close: closes[1] }),
        asCaller: () => ({ caller: "two" }),
      })
      .mockResolvedValueOnce({
        getIo: () => ({ close: closes[2] }),
        asCaller: () => ({ caller: "three" }),
      });

    const mod = await import("./client");
    await mod.getShipClient();
    await mod.getShipClient({ forceNew: true });
    await mod.getShipClient({ forceNew: true });
    mod.invalidateShipClient("done");

    expect(closes[0]).toHaveBeenCalledTimes(1);
    expect(closes[1]).toHaveBeenCalledTimes(1);
    expect(closes[2]).toHaveBeenCalledTimes(1);
  });
});
