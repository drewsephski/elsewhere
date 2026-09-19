import { describe, expect, test } from "vitest";
import { githubConnectorUnavailableMessage } from "./github-oauth";

describe("githubConnectorUnavailableMessage", () => {
  test("maps host configuration errors", () => {
    expect(
      githubConnectorUnavailableMessage("validation: connectors are not configured on this host"),
    ).toBe("GitHub isn't available on this host yet.");
    expect(githubConnectorUnavailableMessage("validation: GitHub App is not configured")).toBe(
      "GitHub isn't available on this host yet.",
    );
    expect(githubConnectorUnavailableMessage("GitHub isn't available on this host")).toBe(
      "GitHub isn't available on this host yet.",
    );
    expect(githubConnectorUnavailableMessage("returnTo must be a bot chat path")).toBeNull();
  });
});
