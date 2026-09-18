// @vitest-environment happy-dom

import { consumeSlackOAuthReturn, rememberSlackOAuthReturn } from "./slack-oauth-return";
import { afterEach, describe, expect, it } from "vitest";

afterEach(() => {
  sessionStorage.clear();
});

describe("slack oauth return", () => {
  it("returns the stored app path and clears it", () => {
    rememberSlackOAuthReturn("/app/connectors");
    expect(consumeSlackOAuthReturn("/app/channels")).toBe("/app/connectors");
    expect(consumeSlackOAuthReturn("/app/channels")).toBe("/app/channels");
  });

  it("ignores stored paths outside /app/", () => {
    rememberSlackOAuthReturn("https://evil.example");
    expect(consumeSlackOAuthReturn("/app/channels")).toBe("/app/channels");
  });
});
