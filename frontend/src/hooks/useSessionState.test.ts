import { describe, expect, it } from "vitest";
import { detectSequenceGap, diagnoseDisconnectReason } from "./useSessionState";

describe("detectSequenceGap", () => {
  it("accepts contiguous replay events", () => {
    let lastSeenSeq: number | null = null;

    for (const nextSeq of [0, 1, 2]) {
      const gap = detectSequenceGap(lastSeenSeq, nextSeq);
      expect(gap).toBeNull();
      lastSeenSeq = nextSeq;
    }
  });

  it("reports a real gap", () => {
    expect(detectSequenceGap(0, 2)).toBe("sequence gap detected: expected 1, received 2");
  });
});

describe("diagnoseDisconnectReason", () => {
  it("classifies a live channel closure as a real disconnect", () => {
    expect(diagnoseDisconnectReason("subscription channel closed")).toBe("real-disconnect");
  });

  it("classifies setup failures and sequence gaps as expected reconnects", () => {
    expect(diagnoseDisconnectReason("subscription setup failed: socket hung up")).toBe(
      "expected-reconnect",
    );
    expect(diagnoseDisconnectReason("sequence gap detected: expected 4, received 8")).toBe(
      "expected-reconnect",
    );
  });
});
