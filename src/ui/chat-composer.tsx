import type { KeyboardEvent } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { SendHorizontal, Square } from "lucide-react";
import { cn } from "@/lib/utils";

interface ChatComposerProps {
  value: string;
  disabled: boolean;
  isStreaming: boolean;
  onChange: (value: string) => void;
  onSend: () => void;
  onStop: () => void;
}

export function ChatComposer({
  value,
  disabled,
  isStreaming,
  onChange,
  onSend,
  onStop,
}: ChatComposerProps) {
  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!disabled && value.trim() && !isStreaming) {
        onSend();
      }
    }
  }

  const canSend = !disabled && value.trim().length > 0 && !isStreaming;

  return (
    <div className="shrink-0 border-t border-border/80 bg-gradient-to-t from-background via-background to-background/80 px-3 pb-3 pt-2 sm:px-4 sm:pb-4">
      <div
        className={cn(
          "mx-auto flex max-w-3xl items-end gap-2 rounded-2xl border border-border/80 bg-card/90 p-2 shadow-sm ring-1 ring-foreground/5 backdrop-blur-sm",
          "focus-within:border-primary/40 focus-within:ring-primary/20",
        )}
      >
        <Textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={disabled && !isStreaming}
          placeholder="Message your bot…"
          rows={1}
          className="min-h-[44px] max-h-40 flex-1 resize-none border-0 bg-transparent px-2 py-2.5 shadow-none focus-visible:ring-0"
          aria-label="Message input"
        />
        {isStreaming ? (
          <Button
            type="button"
            variant="secondary"
            size="icon"
            onClick={onStop}
            aria-label="Stop response"
            className="shrink-0 rounded-xl"
          >
            <Square className="size-4 fill-current" />
          </Button>
        ) : (
          <Button
            type="button"
            size="icon"
            onClick={onSend}
            disabled={!canSend}
            aria-label="Send message"
            className="shrink-0 rounded-xl"
          >
            <SendHorizontal className="size-4" />
          </Button>
        )}
      </div>
      <p className="mx-auto mt-2 max-w-3xl text-center text-[11px] text-muted-foreground">
        Enter to send · Shift+Enter for a new line
      </p>
    </div>
  );
}
