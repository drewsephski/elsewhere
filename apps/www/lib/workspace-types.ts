export type BotPresenceState =
  | "working"
  | "queued"
  | "waiting_approval"
  | "saving_results"
  | "needs_computer"
  | "needs_attention"
  | "ready";

export interface WorkspaceBotPresence {
  id: string;
  name: string;
  avatarId?: string;
  computerName: string | null;
  presence: BotPresenceState | string;
  workId: string | null;
  task: string | null;
}

export interface WorkspaceOverview {
  runnerReady: boolean;
  counts: {
    working: number;
    queued: number;
    approvals: number;
    finished: number;
    results: number;
    routines: number;
  };
  bots: WorkspaceBotPresence[];
}

export const presenceLabels: Record<string, string> = {
  working: "Working",
  queued: "Work queued",
  waiting_approval: "Waiting for you",
  saving_results: "Saving results",
  needs_computer: "Needs a computer",
  needs_attention: "Needs attention",
  ready: "Ready for an assignment",
};

export const presenceShortLabels: Record<string, string> = {
  working: "Working",
  queued: "Queued",
  waiting_approval: "Approve",
  saving_results: "Saving",
  needs_computer: "Computer",
  needs_attention: "Needs you",
  ready: "Ready",
};

export type PresenceTone = "info" | "warning" | "success" | "neutral";

export function presenceTone(presence: string): PresenceTone {
  if (presenceIsActive(presence)) {
    return "info";
  }
  if (presenceNeedsAttention(presence)) {
    return "warning";
  }
  if (presence === "ready") {
    return "success";
  }
  return "neutral";
}

export function presenceDotClass(presence: string): string {
  switch (presenceTone(presence)) {
    case "info":
      return "bg-info";
    case "warning":
      return "bg-warning";
    case "success":
      return "bg-success";
    default:
      return "bg-muted-foreground";
  }
}

export function presenceShortLabel(presence: string): string | null {
  return presenceShortLabels[presence] ?? null;
}

export function presenceNeedsAttention(presence: string): boolean {
  return ["waiting_approval", "needs_attention", "needs_computer"].includes(presence);
}

export function presenceIsActive(presence: string): boolean {
  return ["working", "queued", "saving_results"].includes(presence);
}

/** Sidebar trailing chip: attention and in-progress states beat quiet timestamps. */
export function presenceSidebarTrailing(presence: string): {
  statusLabel: string | null;
  emphasis: "attention" | "active" | null;
} {
  const statusLabel = presenceShortLabel(presence);
  if (presenceNeedsAttention(presence) && statusLabel) {
    return { statusLabel, emphasis: "attention" };
  }
  if (statusLabel && presenceIsActive(presence)) {
    return { statusLabel, emphasis: "active" };
  }
  return { statusLabel: null, emphasis: null };
}

export function activityPreview(bot: WorkspaceBotPresence): string {
  if (bot.task) {
    return bot.task;
  }
  return presenceLabels[bot.presence] ?? "Available";
}
