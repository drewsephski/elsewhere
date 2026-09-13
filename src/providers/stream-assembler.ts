import type { ProviderStreamEvent } from "@/providers/types";

export function applyStreamEvent(
  content: string,
  event: ProviderStreamEvent,
): string {
  if (event.type === "delta" && event.delta) {
    return content + event.delta;
  }
  if (event.type === "done" && event.fullContent !== undefined) {
    return event.fullContent;
  }
  if (event.type === "cancelled" && event.fullContent !== undefined) {
    return event.fullContent;
  }
  return content;
}

export function assembleChunks(chunks: string[]): string {
  return chunks.join("");
}
