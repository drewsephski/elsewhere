import { cloudHostFetch } from "@/lib/cloud-api";

export function canArchiveWorkRun(status: string | undefined): boolean {
  if (!status) {
    return false;
  }
  return status !== "queued" && status !== "running";
}

export async function archiveWorkRun(runId: string): Promise<void> {
  const response = await cloudHostFetch(`/v1/runs/${runId}/archive`, {
    method: "POST",
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? "Could not archive work");
  }
}

export async function deleteConversationMessage(
  conversationId: string,
  messageId: string,
): Promise<void> {
  const response = await cloudHostFetch(
    `/v1/conversations/${conversationId}/messages/${messageId}`,
    { method: "DELETE" },
  );
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? "Could not delete message");
  }
}
