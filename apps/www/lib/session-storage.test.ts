import { afterEach, describe, expect, it } from "vitest";
import {
  readSessionStorage,
  removeSessionStorage,
  writeSessionStorage,
} from "./session-storage";
import {
  consumeOnboardingOffer,
  rememberOnboardingOffer,
} from "./bot-onboarding";
import {
  consumeQuickStartDraft,
  rememberQuickStartDraft,
} from "./bot-quick-start";
import {
  consumeSlackOAuthReturn,
  rememberSlackOAuthReturn,
} from "./slack-oauth-return";

const originalSessionStorage = Object.getOwnPropertyDescriptor(
  globalThis,
  "sessionStorage",
);

function restoreSessionStorage() {
  if (originalSessionStorage) {
    Object.defineProperty(globalThis, "sessionStorage", originalSessionStorage);
    return;
  }
  Reflect.deleteProperty(globalThis, "sessionStorage");
}

function withoutSessionStorage<T>(run: () => T): T {
  Reflect.deleteProperty(globalThis, "sessionStorage");
  try {
    return run();
  } finally {
    restoreSessionStorage();
  }
}

function withUnavailableSessionStorage<T>(run: () => T): T {
  Object.defineProperty(globalThis, "sessionStorage", {
    configurable: true,
    get() {
      throw new Error("sessionStorage is not available");
    },
  });
  try {
    return run();
  } finally {
    restoreSessionStorage();
  }
}

afterEach(() => {
  restoreSessionStorage();
});

describe("session storage without a browser", () => {
  it("does not throw when sessionStorage is missing", () => {
    withoutSessionStorage(() => {
      expect("sessionStorage" in globalThis).toBe(false);
      expect(writeSessionStorage("elsewhere:test", "value")).toBe(false);
      expect(readSessionStorage("elsewhere:test")).toBeNull();
      expect(() => removeSessionStorage("elsewhere:test")).not.toThrow();
    });
  });

  it("does not throw when sessionStorage access is blocked", () => {
    withUnavailableSessionStorage(() => {
      expect(writeSessionStorage("elsewhere:test", "value")).toBe(false);
      expect(readSessionStorage("elsewhere:test")).toBeNull();
      expect(() => removeSessionStorage("elsewhere:test")).not.toThrow();
    });
  });

  it("keeps onboarding, quick-start, and slack helpers fail-safe", () => {
    withoutSessionStorage(() => {
      expect(() => rememberOnboardingOffer("bot_1")).not.toThrow();
      expect(consumeOnboardingOffer("bot_1")).toBe(false);
      expect(() => rememberQuickStartDraft("bot_1", "Write a brief")).not.toThrow();
      expect(consumeQuickStartDraft("bot_1")).toBeNull();
      expect(() => rememberSlackOAuthReturn("/app/connectors")).not.toThrow();
      expect(consumeSlackOAuthReturn("/app/channels")).toBe("/app/channels");
    });
  });
});
