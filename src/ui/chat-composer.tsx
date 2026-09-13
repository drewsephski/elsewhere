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
  function handleKeyDown(event: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!disabled && value.trim()) {
        onSend();
      }
    }
  }

  return (
    <div className="border-t border-border bg-surface-1 p-3">
      <div className="flex gap-2 items-end max-w-4xl mx-auto">
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={disabled && !isStreaming}
          placeholder="Message… (Enter to send, Shift+Enter for newline)"
          rows={3}
          className="flex-1 resize-none rounded-md border border-border bg-surface-0 px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-accent disabled:opacity-50"
          aria-label="Message input"
        />
        {isStreaming ? (
          <button
            type="button"
            onClick={onStop}
            className="shrink-0 rounded-md border border-border bg-surface-2 px-4 py-2 text-sm hover:bg-surface-3"
          >
            Stop
          </button>
        ) : (
          <button
            type="button"
            onClick={onSend}
            disabled={disabled || !value.trim()}
            className="shrink-0 rounded-md bg-accent px-4 py-2 text-sm text-white hover:bg-accent-hover disabled:opacity-40"
          >
            Send
          </button>
        )}
      </div>
    </div>
  );
}
