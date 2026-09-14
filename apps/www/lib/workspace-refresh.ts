import type { WorkspaceEntry } from "@/lib/computer-workspace";

/** Reserved Elsewhere housekeeping entries (mirrors agent-core). */
export function isInternalWorkspaceEntry(name: string): boolean {
  return name.startsWith(".elsewhere-");
}

export function filterPublicWorkspaceEntries(entries: WorkspaceEntry[]): WorkspaceEntry[] {
  return entries.filter((entry) => !isInternalWorkspaceEntry(entry.name));
}

export function workspaceRevisionChanged(
  previous: number | null,
  next: number | undefined,
): boolean {
  if (next === undefined) {
    return false;
  }
  if (previous === null) {
    return false;
  }
  return next !== previous;
}

export function pathsToRefreshOnInvalidation(
  loadedPaths: string[],
  workspaceRoot: string,
): string[] {
  const unique = new Set<string>([workspaceRoot]);
  for (const path of loadedPaths) {
    unique.add(path);
  }
  return [...unique];
}
