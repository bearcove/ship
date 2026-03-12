import { describe, expect, it } from "vitest";
import { diagnoseDisconnectReason } from "./useSessionState";

describe("diagnoseDisconnectReason", () => {
  it("classifies a live channel closure as a real disconnect", () => {
    expect(diagnoseDisconnectReason("subscription channel closed")).toBe("real-disconnect");
  });

  it("classifies setup failures as expected reconnects", () => {
    expect(diagnoseDisconnectReason("subscription setup failed: socket hung up")).toBe(
      "expected-reconnect",
    );
  });

  it("classifies other reasons as other", () => {
    expect(diagnoseDisconnectReason("sequence gap detected: expected 4, received 8")).toBe("other");
  });
});
