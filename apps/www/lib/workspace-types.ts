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

export function presenceNeedsAttention(presence: string): boolean {
  return ["waiting_approval", "needs_attention", "needs_computer"].includes(presence);
}

export function presenceIsActive(presence: string): boolean {
  return ["working", "queued", "saving_results"].includes(presence);
}

export function activityPreview(bot: WorkspaceBotPresence): string {
  if (bot.task) {
    return bot.task;
  }
  return presenceLabels[bot.presence] ?? "Available";
}
