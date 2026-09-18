import { describe, expect, it } from "vitest";
import { runRecoveryGuide } from "./run-recovery";

describe("runRecoveryGuide", () => {
  it("suggests ChatGPT connection for codex auth failures", () => {
    const guide = runRecoveryGuide("failed", "codex_not_authenticated", "run_1");
    expect(guide?.title).toContain("Connect ChatGPT");
    expect(guide?.actions.some((a) => a.kind === "connect_chatgpt")).toBe(true);
  });

  it("returns null for completed runs", () => {
    expect(runRecoveryGuide("completed", null, "run_1")).toBeNull();
  });
});
