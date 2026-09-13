import { describe, expect, test } from "vitest";
import type { Message } from "@/lib/definitions";
import { applyChatStreamEventToMessages } from "@/providers/chat-stream-state";

describe("applyChatStreamEventToMessages", () => {
  test("appends structured tool timeline messages", () => {
    const assistant: Message = {
      id: "assistant-1",
      conversationId: "conv-1",
      role: "assistant",
      kind: "text",
      body: "",
      status: "streaming",
      model: "gpt-5.6-luna",
      errorMessage: null,
      createdAt: 1,
      updatedAt: 1,
    };
    const toolMessage: Message = {
      id: "tool-1",
      conversationId: "conv-1",
      role: "assistant",
      kind: "tool_call",
      body: JSON.stringify({
        tool: "workspace_write",
        callId: "call_1",
        arguments: { path: "/workspace/hello.txt" },
      }),
      status: "complete",
      model: "gpt-5.6-luna",
      errorMessage: null,
      createdAt: 2,
      updatedAt: 2,
    };

    const next = applyChatStreamEventToMessages([assistant], {
      type: "message",
      requestId: "req-1",
      assistantMessageId: "assistant-1",
      message: toolMessage,
    });

    expect(next).toHaveLength(2);
    expect(next[1]?.kind).toBe("tool_call");
  });
});
