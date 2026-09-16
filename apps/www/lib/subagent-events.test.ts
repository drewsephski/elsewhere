import { describe, expect, it } from "vitest";
import {
  isSubagentEvent,
  subagentActivityFromPayload,
  subagentHeadline,
  upsertSubagentActivity,
} from "./subagent-events";

describe("subagent event reduction", () => {
  it("upserts a running helper into a finished card", () => {
    const started = subagentActivityFromPayload({
      subagentId: "sa-1",
      name: "Reviewer",
      taskSummary: "reviewing implementation",
      status: "running",
    });
    expect(started).not.toBeNull();
    const running = upsertSubagentActivity([], started!);
    expect(subagentHeadline(running[0]!)).toBe("Reviewer — reviewing implementation");
    const completed = subagentActivityFromPayload({
      subagentId: "sa-1",
      name: "Reviewer",
      taskSummary: "reviewing implementation",
      status: "completed",
      resultPreview: "Looks good.",
    });
    const finished = upsertSubagentActivity(running, completed!);
    expect(finished).toHaveLength(1);
    expect(subagentHeadline(finished[0]!)).toBe("Reviewer finished");
    expect(finished[0]?.resultPreview).toBe("Looks good.");
  });

  it("recognizes durable subagent events", () => {
    expect(isSubagentEvent("subagent_started")).toBe(true);
    expect(isSubagentEvent("bot_delegation_queued")).toBe(false);
  });
});
