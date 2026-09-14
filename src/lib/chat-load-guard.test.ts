import { describe, expect, test } from "vitest";
import { shouldCommitChatLoad } from "@desktop/lib/chat-load-guard";

describe("chat-load-guard", () => {
  test("commits only when epoch and bot selection match", () => {
    expect(shouldCommitChatLoad(1, 1, "bot-a", "bot-a")).toBe(true);
    expect(shouldCommitChatLoad(1, 2, "bot-a", "bot-a")).toBe(false);
    expect(shouldCommitChatLoad(1, 1, "bot-a", "bot-b")).toBe(false);
    expect(shouldCommitChatLoad(1, 1, "bot-a", null)).toBe(false);
  });
});
