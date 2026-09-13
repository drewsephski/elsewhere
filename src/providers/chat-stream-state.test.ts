import { describe, expect, test } from "vitest";
import type { Message } from "@/lib/definitions";
import { applyChatStreamEventsToMessages } from "@/providers/chat-stream-state";
import type { ProviderStreamEvent } from "@/providers/types";

function assistantMessage(id: string, body = ""): Message {
  return {
    id,
    conversationId: "conv-1",
    role: "assistant",
    kind: "text",
    body,
    status: "streaming",
    model: "gpt-4o-mini",
    errorMessage: null,
    createdAt: 1,
    updatedAt: 1,
  };
}

describe("chat-stream-state", () => {
  test("replays pre-ack deltas onto durable assistant id", () => {
    const events: ProviderStreamEvent[] = [
      {
        type: "delta",
        requestId: "req-1",
        assistantMessageId: "asst-1",
        delta: "Hel",
      },
      {
        type: "delta",
        requestId: "req-1",
        assistantMessageId: "asst-1",
        delta: "lo",
      },
    ];
    const next = applyChatStreamEventsToMessages(
      [assistantMessage("asst-1")],
      events,
    );
    expect(next[0]?.body).toBe("Hello");
  });
});
