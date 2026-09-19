// @vitest-environment happy-dom

import { afterEach, describe, expect, it } from "vitest";
import {
  consumeOnboardingOffer,
  rememberOnboardingOffer,
} from "./bot-onboarding";

afterEach(() => {
  sessionStorage.clear();
});

describe("bot onboarding offer storage", () => {
  it("remembers and consumes a per-bot setup offer", () => {
    rememberOnboardingOffer("bot_1");
    expect(consumeOnboardingOffer("bot_2")).toBe(false);
    expect(consumeOnboardingOffer("bot_1")).toBe(true);
    expect(consumeOnboardingOffer("bot_1")).toBe(false);
  });

  it("ignores malformed stored data", () => {
    sessionStorage.setItem("elsewhere:bot-onboarding-offer", "{not-json");
    expect(consumeOnboardingOffer("bot_1")).toBe(false);
    expect(sessionStorage.getItem("elsewhere:bot-onboarding-offer")).toBeNull();
  });
});
