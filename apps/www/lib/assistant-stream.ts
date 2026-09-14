export type AssistantStreamState = {
  answerText: string;
  commentaryText: string | null;
  streaming: boolean;
};

export type ItemProgressMap = Map<string, number>;

export function emptyAssistantStream(): AssistantStreamState {
  return { answerText: "", commentaryText: null, streaming: false };
}

function endOffsetFromPayload(payload: Record<string, unknown>): number {
  if (typeof payload.endOffset === "number") {
    return payload.endOffset;
  }
  if (typeof payload.cumulativeLength === "number") {
    return payload.cumulativeLength;
  }
  return 0;
}

/** Apply one assistant_delta payload; skips duplicate or overlapping chunks. */
export function applyAssistantDelta(
  stream: AssistantStreamState,
  itemProgress: ItemProgressMap,
  payload: Record<string, unknown>,
): AssistantStreamState {
  const itemId = typeof payload.itemId === "string" ? payload.itemId : "";
  const phase = typeof payload.phase === "string" ? payload.phase : "unknown";
  const delta = typeof payload.delta === "string" ? payload.delta : "";
  const endOffset = endOffsetFromPayload(payload);
  const startOffset =
    typeof payload.startOffset === "number" ? payload.startOffset : null;

  if (!delta) {
    return stream;
  }

  if (itemId) {
    const previous = itemProgress.get(itemId) ?? 0;
    if (endOffset > 0 && endOffset <= previous) {
      return stream;
    }
    if (startOffset !== null && startOffset < previous) {
      return stream;
    }
    if (endOffset > 0) {
      itemProgress.set(itemId, endOffset);
    } else if (startOffset === null) {
      itemProgress.set(itemId, previous + delta.length);
    }
  }

  if (phase === "commentary") {
    const nextCommentary = `${stream.commentaryText ?? ""}${delta}`;
    return {
      ...stream,
      commentaryText: nextCommentary.trim() ? nextCommentary : stream.commentaryText,
      streaming: true,
    };
  }

  return {
    ...stream,
    answerText: `${stream.answerText}${delta}`,
    streaming: true,
  };
}
