import type { ProviderStreamEvent } from "@/providers/types";

/** Events received before `startChat` returns durable message ids. */
export function pushPreAckStreamEvent(
  buffer: ProviderStreamEvent[],
  event: ProviderStreamEvent,
): void {
  buffer.push(event);
}

export function drainPreAckStreamEvents(
  buffer: ProviderStreamEvent[],
): ProviderStreamEvent[] {
  const events = [...buffer];
  buffer.length = 0;
  return events;
}
