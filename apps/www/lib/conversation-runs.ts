/** Matches `LEFT(..., 180)` in list runs when `full_task` is not set. */
export const RUN_TASK_LIST_PREVIEW_MAX = 180;

export function buildConversationRunsListPath(
  botId: string,
  conversationId: string,
  limit = 40,
): string {
  const params = new URLSearchParams({
    limit: String(limit),
    bot_id: botId,
    conversation_id: conversationId,
    full_task: "true",
  });
  return `/v1/runs?${params.toString()}`;
}

export function buildRunsListPreviewPath(botId?: string, limit = 50): string {
  const params = new URLSearchParams({ limit: String(limit) });
  if (botId) {
    params.set("bot_id", botId);
  }
  return `/v1/runs?${params.toString()}`;
}

export function retryPrefillText(run: { task?: string | null; status: string }): string {
  const trimmed = run.task?.trim() ?? "";
  if (run.status === "interrupted" && trimmed) {
    return `Please continue where you left off: ${trimmed}`;
  }
  return trimmed;
}

export function truncateRunTaskPreview(task: string): string {
  if (task.length <= RUN_TASK_LIST_PREVIEW_MAX) {
    return task;
  }
  return task.slice(0, RUN_TASK_LIST_PREVIEW_MAX);
}
