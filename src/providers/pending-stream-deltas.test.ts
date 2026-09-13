import { describe, expect, test } from "vitest";
import {
  bufferStreamDelta,
  takeBufferedStreamBody,
} from "@/providers/pending-stream-deltas";

describe("pending-stream-deltas", () => {
  test("buffers and drains by assistant message id", () => {
    const pending = new Map<string, string>();
    bufferStreamDelta(pending, "asst-1", "Hel");
    bufferStreamDelta(pending, "asst-1", "lo");
    expect(takeBufferedStreamBody(pending, "asst-1")).toBe("Hello");
    expect(pending.size).toBe(0);
  });
});
