import { describe, expect, test } from "vitest";
import { shouldCommitChatLoad } from "@/lib/chat-load-guard";

describe("chat-load-guard", () => {
  test("commits only when epoch matches", () => {
    expect(shouldCommitChatLoad(1, 1)).toBe(true);
    expect(shouldCommitChatLoad(1, 2)).toBe(false);
  });
});
