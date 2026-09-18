import type { WorkspaceEntry } from "@/lib/computer-workspace";

/** Reserved Elsewhere housekeeping entries (mirrors agent-core). */
export function isInternalWorkspaceEntry(name: string): boolean {
  return name === ".elsewhere" || name.startsWith(".elsewhere-");
}

const HIDDEN_LISTING_NAMES = new Set([
  ".elsewhere",
  ".ds_store",
  ".trashes",
  ".spotlight-v100",
  ".fseventsd",
  "thumbs.db",
]);

const HIDDEN_LISTING_DIR_NAMES = new Set([
  ".git",
  "__macosx",
  "node_modules",
  ".next",
  "dist",
  "build",
  "target",
  "coverage",
]);

export function isHiddenWorkspaceListingName(name: string, isDir: boolean): boolean {
  if (isInternalWorkspaceEntry(name)) {
    return true;
  }
  const key = name.toLowerCase();
  if (HIDDEN_LISTING_NAMES.has(key)) {
    return true;
  }
  return isDir && HIDDEN_LISTING_DIR_NAMES.has(key);
}

function workspacePathSegments(path: string): string[] {
  return path.split("/").filter((segment) => segment.length > 0 && segment !== "workspace");
}

function pathHasHiddenDirectoryAncestor(path: string): boolean {
  const segments = workspacePathSegments(path);
  if (segments.length < 2) {
    return false;
  }
  return segments.slice(0, -1).some((segment) => isHiddenWorkspaceListingName(segment, true));
}

/** User-facing Files visibility. Presentation/filtering only — files are not deleted. */
export function isUserVisibleWorkspaceEntry(path: string, name: string, isDir: boolean): boolean {
  if (isHiddenWorkspaceListingName(name, isDir)) {
    return false;
  }
  return !pathHasHiddenDirectoryAncestor(path);
}

export function filterPublicWorkspaceEntries(entries: WorkspaceEntry[]): WorkspaceEntry[] {
  return entries.filter((entry) => isUserVisibleWorkspaceEntry(entry.path, entry.name, entry.isDir));
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
