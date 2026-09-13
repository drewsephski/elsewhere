const WORKSPACE_NAME_KEY = "gptbot.workspace.name";
export const DEFAULT_WORKSPACE_NAME = "Clearmud";

export function readWorkspaceName(): string {
  try {
    const raw = localStorage.getItem(WORKSPACE_NAME_KEY);
    const trimmed = raw?.trim();
    return trimmed || DEFAULT_WORKSPACE_NAME;
  } catch {
    return DEFAULT_WORKSPACE_NAME;
  }
}

export function writeWorkspaceName(name: string): void {
  const trimmed = name.trim();
  if (!trimmed) {
    return;
  }
  try {
    localStorage.setItem(WORKSPACE_NAME_KEY, trimmed);
  } catch {
    /* ignore */
  }
}
