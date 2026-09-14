import { describe, expect, it } from "vitest";
import type { WorkspaceLoadPhase } from "../hooks/use-workspace-overview";
import {
  workspaceBotsEmptyMessage,
  workspaceBotsForEmptyList,
} from "./workspace-load-state";

describe("workspace load phases", () => {
  it("does not treat initial outage as an empty account", () => {
    const phase: WorkspaceLoadPhase = "unavailable";
    const label = workspaceBotsEmptyMessage(phase, {
      workspaceError: "Runner unavailable",
    });
    expect(label).toBe("Runner unavailable");
  });

  it("preserves stale workspace data semantics", () => {
    const previousBots = [{ id: "1" }];
    const phase: WorkspaceLoadPhase = "stale";
    const bots = workspaceBotsForEmptyList(phase, previousBots);
    expect(bots).toHaveLength(1);
  });

  it("shows loading copy while the first fetch is in flight", () => {
    expect(workspaceBotsEmptyMessage("initial")).toBe("Loading your bots…");
    expect(workspaceBotsEmptyMessage("loading")).toBe("Loading your bots…");
  });
});
