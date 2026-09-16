export type SubagentActivity = {
  subagentId: string;
  name: string;
  taskSummary: string;
  status: string;
  resultPreview?: string;
  error?: string;
};

export function isSubagentEvent(event: string): boolean {
  return (
    event === "subagent_started" ||
    event === "subagent_completed" ||
    event === "subagent_failed" ||
    event === "subagent_cancelled"
  );
}

export function subagentActivityFromPayload(
  payload: Record<string, unknown>,
): SubagentActivity | null {
  const subagentId = typeof payload.subagentId === "string" ? payload.subagentId : "";
  const name = typeof payload.name === "string" ? payload.name : "";
  if (!subagentId || !name) {
    return null;
  }
  const taskSummary =
    typeof payload.taskSummary === "string" ? payload.taskSummary : "";
  const status = typeof payload.status === "string" ? payload.status : "running";
  const resultPreview =
    typeof payload.resultPreview === "string" ? payload.resultPreview : undefined;
  const error = typeof payload.error === "string" ? payload.error : undefined;
  return { subagentId, name, taskSummary, status, resultPreview, error };
}

export function upsertSubagentActivity(
  current: SubagentActivity[],
  next: SubagentActivity,
): SubagentActivity[] {
  const index = current.findIndex((item) => item.subagentId === next.subagentId);
  if (index < 0) {
    return [...current, next];
  }
  const copy = current.slice();
  copy[index] = { ...copy[index], ...next };
  return copy;
}

export function subagentHeadline(activity: SubagentActivity): string {
  if (activity.status === "running") {
    const summary = activity.taskSummary.trim();
    return summary ? `${activity.name} — ${summary}` : `${activity.name} — working`;
  }
  if (activity.status === "completed") {
    return `${activity.name} finished`;
  }
  if (activity.status === "failed") {
    return `${activity.name} failed`;
  }
  if (activity.status === "cancelled" || activity.status === "interrupted") {
    return `${activity.name} stopped`;
  }
  return activity.name;
}
