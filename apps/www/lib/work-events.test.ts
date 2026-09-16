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

  it("does not flatten subagent events into generic timeline text", () => {
    expect(
      activityText("subagent_started", {
        subagentId: "sa-1",
        name: "Reviewer",
        status: "running",
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
