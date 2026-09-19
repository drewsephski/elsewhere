import { describe, expect, test } from "vitest";
import { replayRunStreamEvents } from "./run-event-timeline";

describe("replayRunStreamEvents", () => {
  test("reconstructs approval and question resolution from durable events", () => {
    const state = replayRunStreamEvents("run_1", [
      {
        id: "e1",
        event: "user_question_requested",
        data: JSON.stringify({
          questionId: "q1",
          question: "Deploy where?",
          options: ["Production", "Staging"],
        }),
      },
      {
        id: "e2",
        event: "user_question_answered",
        data: JSON.stringify({ questionId: "q1", selectedIndex: 1 }),
      },
      {
        id: "e3",
        event: "approval_requested",
        data: JSON.stringify({
          approvalId: "a1",
          summary: "Update README.md",
          tool: "workspace_write",
          operationKind: "write",
        }),
      },
      {
        id: "e4",
        event: "approval_resolved",
        data: JSON.stringify({ approvalId: "a1", decision: "approved" }),
      },
    ]);

    const question = state.items.find((item) => item.kind === "question");
    expect(question?.kind).toBe("question");
    if (question?.kind === "question") {
      expect(question.question.status).toBe("answered");
      expect(question.question.selectedIndex).toBe(1);
    }

    const approval = state.items.find((item) => item.kind === "approval");
    expect(approval?.kind).toBe("approval");
    if (approval?.kind === "approval") {
      expect(approval.decision).toBe("approved");
    }
  });

  test("does not duplicate timeline items when the same event id is replayed twice", () => {
    const events = [
      {
        id: "dup",
        event: "tool_started",
        data: JSON.stringify({ tool: "workspace_read", name: "read" }),
      },
    ];
    const once = replayRunStreamEvents("run_1", events);
    const twice = replayRunStreamEvents("run_1", [...events, ...events]);
    expect(once.items.filter((item) => item.kind === "tool").length).toBe(1);
    expect(twice.items.filter((item) => item.kind === "tool").length).toBe(1);
  });

  test("replays connector_needed then resolved as one resolved card", () => {
    const state = replayRunStreamEvents("run_1", [
      {
        id: "c1",
        event: "connector_needed",
        data: JSON.stringify({
          needId: "cneed_1",
          provider: "github",
          reason: { kind: "disconnected" },
          runId: "run_1",
          botId: "bot_1",
          toolName: "github_list_repositories",
          requestedAt: "2026-09-19T17:00:00.000Z",
        }),
      },
      {
        id: "c1-dup",
        event: "connector_needed",
        data: JSON.stringify({
          needId: "cneed_1",
          provider: "github",
          reason: { kind: "disconnected" },
          runId: "run_1",
          botId: "bot_1",
          toolName: "github_list_repositories",
          requestedAt: "2026-09-19T17:00:00.000Z",
        }),
      },
      {
        id: "c2",
        event: "connector_needed_resolved",
        data: JSON.stringify({ needId: "cneed_1", resolution: "connected" }),
      },
    ]);
    const connectors = state.items.filter((item) => item.kind === "connector");
    expect(connectors.length).toBe(1);
    if (connectors[0]?.kind === "connector") {
      expect(connectors[0].need.status).toEqual({
        phase: "resolved",
        resolution: "connected",
      });
    }
  });

  test("does not invent a connector card from a historical tool_result error", () => {
    const state = replayRunStreamEvents("run_1", [
      {
        id: "t1",
        event: "tool_result",
        data: JSON.stringify({
          tool: "github_list_repositories",
          ok: false,
          error: "Listing GitHub repositories did not complete",
        }),
      },
    ]);
    expect(state.items.some((item) => item.kind === "connector")).toBe(false);
    expect(state.items.some((item) => item.kind === "tool")).toBe(true);
  });
});
