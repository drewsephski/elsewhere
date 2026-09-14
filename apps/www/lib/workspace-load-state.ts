import type { WorkspaceLoadPhase } from "@/hooks/use-workspace-overview";

export function workspaceBotsEmptyMessage(
  phase: WorkspaceLoadPhase,
  options: { query?: string; workspaceError?: string | null } = {},
): string {
  if (phase === "initial" || phase === "loading") {
    return "Loading your bots…";
  }
  if (phase === "unavailable" || phase === "stale") {
    return options.workspaceError ?? "Workspace runner is temporarily unavailable.";
  }
  if (options.query) {
    return "No bots match your search.";
  }
  return "No bots yet.";
}

export function workspaceBotsForEmptyList<T>(
  phase: WorkspaceLoadPhase,
  previousBots: T[],
): T[] {
  return phase === "stale" ? previousBots : [];
}
