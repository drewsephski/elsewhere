import { describe, expect, test } from "vitest";
import { resolveRoutineComposePrefill } from "./routine-compose-prefill";
import type { BotSummary, ConversationSummary } from "@/lib/api-types";

const bots: BotSummary[] = [
  {
    id: "bot-a",
    name: "Scout",
    role: "",
    model: "gpt-5.6-luna",
    computerId: "comp-a",
    engine: "codex",
    avatarSlug: "sky-wisp",
    learnFromConversations: false,
  },
];

const conversations: ConversationSummary[] = [
  {
    id: "conv-direct",
    botId: "bot-a",
    conversationType: "direct",
    name: null,
    updatedAt: "2026-01-01T00:00:00.000Z",
  },
  {
    id: "conv-group",
    botId: "bot-a",
    conversationType: "group",
    name: "Ops",
    updatedAt: "2026-01-01T00:00:00.000Z",
  },
];

describe("resolveRoutineComposePrefill", () => {
  test("returns null for unknown bot", () => {
    expect(resolveRoutineComposePrefill(bots, conversations, "missing", null)).toBeNull();
  });

  test("prefills bot only with direct-chat default destination", () => {
    expect(resolveRoutineComposePrefill(bots, conversations, "bot-a", null)).toEqual({
      botId: "bot-a",
      destinationConversationId: "",
    });
  });

  test("prefills group destination when conversation is valid", () => {
    expect(resolveRoutineComposePrefill(bots, conversations, "bot-a", "conv-group")).toEqual({
      botId: "bot-a",
      destinationConversationId: "conv-group",
    });
  });

  test("ignores conversation that does not belong to the bot", () => {
    const foreign: ConversationSummary[] = [
      {
        id: "conv-other",
        botId: "other-bot",
        conversationType: "direct",
        name: null,
        updatedAt: "2026-01-01T00:00:00.000Z",
      },
    ];
    expect(resolveRoutineComposePrefill(bots, foreign, "bot-a", "conv-other")).toEqual({
      botId: "bot-a",
      destinationConversationId: "",
    });
  });
});
