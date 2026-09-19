import { describe, expect, test } from "vitest";
import {
  botChatReturnTo,
  parseBotChatReturnTo,
  parseConnectorNeed,
  presentationForConnectorNeed,
} from "./connector-need";

describe("parseBotChatReturnTo", () => {
  test("accepts bot chat paths with optional conversation", () => {
    expect(parseBotChatReturnTo("/app/bots/bot_1")).toBe("/app/bots/bot_1");
    expect(parseBotChatReturnTo("/app/bots/bot_1?conversation=c1")).toBe(
      "/app/bots/bot_1?conversation=c1",
    );
  });

  test("rejects connectors, foreign origins, and other app paths", () => {
    expect(parseBotChatReturnTo("/app/connectors")).toBeNull();
    expect(parseBotChatReturnTo("https://evil.example")).toBeNull();
    expect(parseBotChatReturnTo("//evil")).toBeNull();
    expect(parseBotChatReturnTo("/app/work/x")).toBeNull();
  });
});

describe("parseConnectorNeed", () => {
  test("parses each reason kind and requires owner plus repo", () => {
    const base = {
      needId: "cneed_1",
      provider: "github",
      runId: "run_1",
      botId: "bot_1",
      toolName: "github_list_repositories",
      requestedAt: "2026-09-19T17:00:00.000Z",
    };
    expect(parseConnectorNeed("run_1", { ...base, reason: { kind: "disconnected" } })?.reason).toEqual({
      kind: "disconnected",
    });
    expect(
      parseConnectorNeed("run_1", { ...base, reason: { kind: "reconnect_required" } })?.reason,
    ).toEqual({ kind: "reconnect_required" });
    expect(
      parseConnectorNeed("run_1", { ...base, reason: { kind: "empty_authorization" } })?.reason,
    ).toEqual({ kind: "empty_authorization" });
    expect(
      parseConnectorNeed("run_1", {
        ...base,
        reason: { kind: "unauthorized_repo", owner: "acme", repo: "elsewhere" },
      })?.reason,
    ).toEqual({ kind: "unauthorized_repo", owner: "acme", repo: "elsewhere" });
    expect(
      parseConnectorNeed("run_1", { ...base, reason: { kind: "unauthorized_repo", owner: "acme" } }),
    ).toBeNull();
  });
});

describe("presentationForConnectorNeed", () => {
  const need = {
    needId: "cneed_1",
    provider: "github" as const,
    runId: "run_1",
    botId: "bot_1",
    toolName: "github_list_repositories",
    requestedAt: "2026-09-19T17:00:00.000Z",
    status: { phase: "pending" as const },
  };

  test("uses distinct copy for each reason", () => {
    expect(presentationForConnectorNeed({ ...need, reason: { kind: "disconnected" } })).toMatchObject({
      title: "Connect GitHub?",
      actionLabel: "Connect GitHub",
    });
    expect(
      presentationForConnectorNeed({ ...need, reason: { kind: "reconnect_required" } }),
    ).toMatchObject({
      title: "Reconnect GitHub?",
      actionLabel: "Reconnect GitHub",
    });
    expect(
      presentationForConnectorNeed({
        ...need,
        reason: { kind: "unauthorized_repo", owner: "acme", repo: "elsewhere" },
      }),
    ).toMatchObject({
      title: "Add this repository?",
      actionLabel: "Add acme on GitHub",
    });
    expect(
      presentationForConnectorNeed({ ...need, reason: { kind: "empty_authorization" } }),
    ).toMatchObject({
      title: "Grant repository access?",
      actionLabel: "Add repositories",
    });
  });
});

describe("botChatReturnTo", () => {
  test("constructs a branded bot chat path", () => {
    expect(botChatReturnTo({ botId: "bot_1", conversationId: "c1" })).toBe(
      "/app/bots/bot_1?conversation=c1",
    );
  });
});
