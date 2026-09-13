import { describe, expect, test } from "vitest";
import {
  drainPreAckStreamEvents,
  pushPreAckStreamEvent,
} from "@/providers/pre-ack-stream-buffer";
import type { ProviderStreamEvent } from "@/providers/types";

describe("pre-ack-stream-buffer", () => {
  test("preserves event order until drained", () => {
    const buffer: ProviderStreamEvent[] = [];
    const delta: ProviderStreamEvent = {
      type: "delta",
      requestId: "req-1",
      assistantMessageId: "asst-1",
      delta: "x",
    };
    const done: ProviderStreamEvent = {
      type: "done",
      requestId: "req-1",
      assistantMessageId: "asst-1",
      fullContent: "x",
    };
    pushPreAckStreamEvent(buffer, delta);
    pushPreAckStreamEvent(buffer, done);
    expect(drainPreAckStreamEvents(buffer)).toEqual([delta, done]);
    expect(buffer).toHaveLength(0);
  });
});
