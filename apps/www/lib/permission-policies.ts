export type PolicyDecision = "allow" | "ask" | "deny";

export interface PolicyAction {
  action: string;
  label: string;
  group: string;
  groupLabel: string;
  decision: PolicyDecision;
  source: "bot" | "owner" | "default" | string;
  inherited: boolean;
  inheritedDecision: PolicyDecision;
  overridable: boolean;
}

export interface PolicyCatalog {
  actions: PolicyAction[];
}

export const POLICY_DECISIONS: { value: PolicyDecision; label: string }[] = [
  { value: "allow", label: "Allow" },
  { value: "ask", label: "Ask" },
  { value: "deny", label: "Deny" },
];

export function groupedPolicyActions(actions: PolicyAction[]): {
  group: string;
  groupLabel: string;
  actions: PolicyAction[];
}[] {
  const order = ["files", "terminal", "browser", "delegation", "connected_apps"];
  const groups = new Map<string, { group: string; groupLabel: string; actions: PolicyAction[] }>();
  for (const action of actions) {
    const existing = groups.get(action.group);
    if (existing) {
      existing.actions.push(action);
    } else {
      groups.set(action.group, {
        group: action.group,
        groupLabel: action.groupLabel,
        actions: [action],
      });
    }
  }
  return order
    .map((key) => groups.get(key))
    .filter((group): group is { group: string; groupLabel: string; actions: PolicyAction[] } =>
      Boolean(group),
    );
}
