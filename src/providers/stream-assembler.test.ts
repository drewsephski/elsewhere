import { describe, expect, test } from "vitest";
import { applyStreamEvent, assembleChunks } from "@/providers/stream-assembler";

describe("stream-assembler", () => {
  test("assembles delta chunks", () => {
    let content = "";
    content = applyStreamEvent(content, {
      type: "delta",
      requestId: "r1",
      assistantMessageId: "m1",
      delta: "Hello",
    });
    content = applyStreamEvent(content, {
      type: "delta",
      requestId: "r1",
      assistantMessageId: "m1",
      delta: ", world",
    });
    expect(content).toBe("Hello, world");
  });

  test("done event sets full content", () => {
    const result = applyStreamEvent("partial", {
      type: "done",
      requestId: "r1",
      assistantMessageId: "m1",
      fullContent: "final",
    });
    expect(result).toBe("final");
  });

  test("assembleChunks joins strings", () => {
    expect(assembleChunks(["a", "b", "c"])).toBe("abc");
  });
});
