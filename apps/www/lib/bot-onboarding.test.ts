import { describe, expect, it } from "vitest";
import {
  botConversationHref,
  consumeOnboardingOffer,
  parseOnboardingState,
  rememberOnboardingOffer,
} from "./bot-onboarding";

describe("bot onboarding helpers", () => {
  it("routes created bots into setup without changing the started-task path", () => {
    expect(botConversationHref("bot_1", { setup: true })).toBe("/app/bots/bot_1?setup=1");
    expect(botConversationHref("bot_1")).toBe("/app/bots/bot_1");
  });

  it("does not throw when remembering or consuming a setup offer without a browser", () => {
    const original = Object.getOwnPropertyDescriptor(globalThis, "sessionStorage");
    Reflect.deleteProperty(globalThis, "sessionStorage");
    try {
      expect(() => rememberOnboardingOffer("bot_1")).not.toThrow();
      expect(consumeOnboardingOffer("bot_1")).toBe(false);
    } finally {
      if (original) {
        Object.defineProperty(globalThis, "sessionStorage", original);
      }
    }
  });

  it("parses resume state and ignores malformed payloads", () => {
    expect(parseOnboardingState({ botId: "bot_1", status: "in_progress", revision: 4 })).toMatchObject({
      botId: "bot_1",
      status: "in_progress",
      revision: 4,
    });
    expect(parseOnboardingState({})).toBeNull();
  });
});
