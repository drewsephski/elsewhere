import { describe, expect, it } from "vitest";
import { activityLineFromEvent } from "./run-activity";

describe("activityLineFromEvent", () => {
  it("maps tool events to human headlines with optional technical names", () => {
    const line = activityLineFromEvent("tool_invoked", {
      tool: "workspace_read",
    });
    expect(line?.headline).toBe("Reading a file");
    expect(line?.technical).toBe("workspace_read");
  });

  it("returns null for assistant stream noise", () => {
    expect(activityLineFromEvent("assistant_delta", {})).toBeNull();
  });
});
