import type { FormEvent } from "react";
import {
  ChatComposerFrame,
  ChatComposerTextarea,
  ComposerIconButton,
} from "@/components/app/workspace/chat-composer";
import { Square } from "@desktop/components/icons/lucide";

interface ChatComposerProps {
  value: string;
  disabled: boolean;
  isStreaming: boolean;
  recipientName?: string | null;
  onChange: (value: string) => void;
  onSend: () => void;
  onStop: () => void;
}

export function ChatComposer({
  value,
  disabled,
  isStreaming,
  recipientName,
  onChange,
  onSend,
  onStop,
}: ChatComposerProps) {
  const placeholder = recipientName
    ? `Message ${recipientName}`
    : "Message your bot";

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (disabled || isStreaming || !value.trim()) {
      return;
    }
    onSend();
  }

  const canSend = !disabled && !isStreaming && Boolean(value.trim());

  return (
    <footer className="shrink-0 px-3 pb-3 pt-1 sm:px-5">
      <div className="mx-auto max-w-3xl">
        <ChatComposerFrame
          onSubmit={handleSubmit}
          canSend={canSend}
          pending={false}
          sendLabel="Send message"
          leading={
            isStreaming ? (
              <ComposerIconButton
                label="Stop response"
                type="button"
                onClick={onStop}
                className="text-destructive hover:text-destructive"
              >
                <Square className="size-3.5 fill-current" aria-hidden />
              </ComposerIconButton>
            ) : undefined
          }
        >
          <ChatComposerTextarea
            value={value}
            onChange={(event) => onChange(event.target.value)}
            disabled={disabled || isStreaming}
            placeholder={placeholder}
            aria-label="Message"
          />
        </ChatComposerFrame>
      </div>
    </footer>
  );
}
