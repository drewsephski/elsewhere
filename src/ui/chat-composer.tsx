import type { KeyboardEvent } from "react";
import { Button } from "@desktop/components/ui/button";
import { Input } from "@desktop/components/ui/input";
import { Mic, Plus, Square } from "@desktop/components/icons/lucide";
import { cn } from "@desktop/lib/utils";

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
  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!disabled && value.trim() && !isStreaming) {
        onSend();
      }
    }
  }

  const placeholder = recipientName
    ? `Message ${recipientName}`
    : "Message your assistant";

  return (
    <div className="shrink-0 border-t border-border/50 bg-white px-4 py-4">
      <div
        className={cn(
          "mx-auto flex max-w-2xl items-center gap-1 rounded-full border border-border/70 bg-[#f5f5f7] px-2 py-1.5 shadow-sm",
          "focus-within:border-foreground/20 focus-within:ring-2 focus-within:ring-foreground/5",
        )}
      >
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="shrink-0 rounded-full text-muted-foreground"
          aria-label="Add attachment"
          disabled
        >
          <Plus className="size-4" />
        </Button>

        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={disabled || isStreaming}
          placeholder={placeholder}
          className="h-9 flex-1 border-0 bg-transparent px-1 shadow-none focus-visible:ring-0"
          aria-label="Message input"
        />

        {isStreaming ? (
          <Button
            type="button"
            variant="secondary"
            size="icon-sm"
            onClick={onStop}
            aria-label="Stop response"
            className="shrink-0 rounded-full"
          >
            <Square className="size-3.5 fill-current" />
          </Button>
        ) : (
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="shrink-0 rounded-full text-muted-foreground"
            aria-label="Voice input"
            disabled
          >
            <Mic className="size-4" />
          </Button>
        )}
      </div>
    </div>
  );
}
