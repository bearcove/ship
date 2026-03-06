import { describe, expect, it } from "vitest";
import { detectSequenceGap } from "./useSessionState";

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
