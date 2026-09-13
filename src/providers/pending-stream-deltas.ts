/** Buffers stream text keyed by durable assistant message id until the UI maps temp ids. */
export function bufferStreamDelta(
  pending: Map<string, string>,
  assistantMessageId: string,
  delta: string,
): void {
  if (!delta) {
    return;
  }
  pending.set(
    assistantMessageId,
    `${pending.get(assistantMessageId) ?? ""}${delta}`,
  );
}

export function takeBufferedStreamBody(
  pending: Map<string, string>,
  assistantMessageId: string,
): string {
  const body = pending.get(assistantMessageId) ?? "";
  pending.delete(assistantMessageId);
  return body;
}
