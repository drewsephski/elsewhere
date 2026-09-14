export interface WorkspaceEntry {
  name: string;
  path: string;
  isDir: boolean;
}

export interface WorkspaceListResponse {
  path: string;
  entries: WorkspaceEntry[];
}

export interface WorkspaceFileResponse {
  path: string;
  size: number;
  isBinary: boolean;
  text: string | null;
}

export const WORKSPACE_ROOT = "/workspace";

export function sortWorkspaceEntries(entries: WorkspaceEntry[]): WorkspaceEntry[] {
  return [...entries].sort((a, b) => {
    if (a.isDir !== b.isDir) {
      return a.isDir ? -1 : 1;
    }
    return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  });
}

export function workspaceFileName(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
