import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import { streamEventSchema, type StreamEvent } from "@/lib/definitions";
import type { ProviderStreamEvent } from "@/providers/types";

const STREAM_EVENT = "gptbot://chat-stream";

export function useChatStreamListener(
  onEvent: (event: ProviderStreamEvent) => void,
) {
  const handlerRef = useRef(onEvent);
  handlerRef.current = onEvent;

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<StreamEvent>(STREAM_EVENT, (payload) => {
      const parsed = streamEventSchema.safeParse(payload.payload);
      if (!parsed.success) {
        return;
      }
      const data = parsed.data;
      const type = data.eventType as ProviderStreamEvent["type"];
      handlerRef.current({
        type,
        requestId: data.requestId,
        assistantMessageId: data.assistantMessageId,
        delta: data.delta ?? undefined,
        error: data.error ?? undefined,
        fullContent: data.fullContent ?? undefined,
        message: data.message ?? undefined,
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);
}
