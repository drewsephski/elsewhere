import { describe, expect, it } from "vitest";
import { isRunnerUnreachableStatus } from "./cloud-bff-errors";

describe("isRunnerUnreachableStatus", () => {
  it("detects structured runner outage responses", () => {
    expect(
      isRunnerUnreachableStatus(503, {
        code: "workspace_upstream_unreachable",
        error: "Workspace runner is temporarily unreachable.",
      }),
    ).toBe(true);
  });

  it("does not treat auth failures as runner outages", () => {
    expect(isRunnerUnreachableStatus(401, { code: "unauthorized" })).toBe(false);
    expect(isRunnerUnreachableStatus(503, { code: "other" })).toBe(false);
  });
});
