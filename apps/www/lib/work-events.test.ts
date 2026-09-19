import { describe, expect, it } from "vitest";
import { activityText } from "./work-events";
import { groupedPolicyActions, type PolicyAction } from "./permission-policies";

describe("permission policy activity", () => {
  it("hides allow decisions from the user-facing timeline", () => {
    expect(
      activityText("permission_policy_resolved", {
        tool: "workspace_write",
        decision: "allow",
        source: "bot",
      }),
    ).toBeNull();
  });

  it("surfaces deny decisions without dumping arguments", () => {
    expect(
      activityText("permission_policy_resolved", {
        tool: "workspace_write",
        decision: "deny",
        source: "owner",
      }),
    ).toBe("Blocked: Writing a file");
  });

  it("humanizes approval resolutions", () => {
    expect(activityText("approval_resolved", { decision: "approved" })).toBe(
      "You allowed the action",
    );
  });

  it("labels GitHub coding tools in the activity timeline", () => {
    expect(activityText("tool_call", { tool: "github_open_repository" })).toBe(
      "Opening repository",
    );
    expect(activityText("tool_call", { tool: "github_publish_pull_request" })).toBe(
      "Opening pull request",
    );
    expect(activityText("tool_call", { tool: "github_update_pull_request" })).toBe(
      "Updating pull request",
    );
  });

  it("labels skill tools in the activity timeline", () => {
    expect(activityText("tool_call", { tool: "skill_list" })).toBe("Checking skills");
    expect(
      activityText("tool_call", {
        tool: "skill_save_recent_work",
        arguments: { name: "Competitor brief" },
      }),
    ).toBe("Saving a skill (Competitor brief)");
  });

  it("labels routine tools in the activity timeline", () => {
    expect(activityText("tool_call", { tool: "routine_list" })).toBe("Checking routines");
    expect(
      activityText("tool_call", {
        tool: "routine_create",
        arguments: { name: "Morning brief" },
      }),
    ).toBe("Scheduling recurring work (Morning brief)");
    expect(
      activityText("tool_call", {
        tool: "routine_pause",
        arguments: { routineName: "Morning brief" },
      }),
    ).toBe("Pausing routine (Morning brief)");
    expect(
      activityText("tool_call", {
        tool: "routine_resume",
        arguments: { routineName: "Morning brief" },
      }),
    ).toBe("Resuming routine (Morning brief)");
  });

  it("does not flatten user questions into generic timeline text", () => {
    expect(
      activityText("user_question_requested", {
        questionId: "q1",
        question: "Pick one?",
        options: ["A", "B"],
      }),
    ).toBeNull();
  });
});

describe("groupedPolicyActions", () => {
  it("keeps a compact group order", () => {
    const actions: PolicyAction[] = [
      {
        action: "bot_delegate",
        label: "Hand off work",
        group: "delegation",
        groupLabel: "Delegation",
        decision: "ask",
        source: "default",
        inherited: true,
        inheritedDecision: "ask",
        overridable: true,
      },
      {
        action: "workspace_write",
        label: "Write files",
        group: "files",
        groupLabel: "Files",
        decision: "ask",
        source: "default",
        inherited: true,
        inheritedDecision: "ask",
        overridable: true,
      },
    ];
    expect(groupedPolicyActions(actions).map((group) => group.group)).toEqual([
      "files",
      "delegation",
    ]);
  });
});
