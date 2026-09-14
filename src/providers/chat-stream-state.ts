import type { Message } from "@desktop/lib/definitions";
import type { ProviderStreamEvent } from "@desktop/providers/types";
import { applyStreamEvent } from "@desktop/providers/stream-assembler";

export function applyChatStreamEventToMessages(
  messages: Message[],
  event: ProviderStreamEvent,
): Message[] {
  if (event.type === "message" && event.message) {
    const existing = messages.some((m) => m.id === event.message!.id);
    if (existing) {
      return messages.map((m) =>
        m.id === event.message!.id ? event.message! : m,
      );
    }
    return [...messages, event.message];
  }
  if (event.type === "error") {
    return messages;
  }
  const hasTarget = messages.some((m) => m.id === event.assistantMessageId);
  if (!hasTarget) {
    return messages;
  }
  return messages.map((m) => {
    if (m.id !== event.assistantMessageId) {
      return m;
    }
    return {
      ...m,
      body: applyStreamEvent(m.body, event),
    };
  });
}

export function applyChatStreamEventsToMessages(
  messages: Message[],
  events: ProviderStreamEvent[],
): Message[] {
  let next = messages;
  for (const event of events) {
    next = applyChatStreamEventToMessages(next, event);
  }
  return next;
}
