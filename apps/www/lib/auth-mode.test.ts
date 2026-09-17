import { describe, expect, it } from "vitest";
import { authModeFromPathname, authModeToggleHref } from "./auth-mode";

describe("authModeFromPathname", () => {
  it("defaults to sign-in for missing or unknown paths", () => {
    expect(authModeFromPathname(null)).toBe("sign-in");
    expect(authModeFromPathname(undefined)).toBe("sign-in");
    expect(authModeFromPathname("/sign-in")).toBe("sign-in");
    expect(authModeFromPathname("/sign-in/")).toBe("sign-in");
    expect(authModeFromPathname("/elsewhere")).toBe("sign-in");
  });

  it("detects sign-up from /sign-up", () => {
    expect(authModeFromPathname("/sign-up")).toBe("sign-up");
    expect(authModeFromPathname("/sign-up/")).toBe("sign-up");
  });
});

describe("authModeToggleHref", () => {
  it("toggles between /sign-in and /sign-up", () => {
    expect(authModeToggleHref("sign-in")).toBe("/sign-up");
    expect(authModeToggleHref("sign-up")).toBe("/sign-in");
  });
});
