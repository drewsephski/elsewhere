import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Message } from "@/lib/definitions";

interface MessageBubbleProps {
  message: Message;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === "user";
  const isError = message.status === "error";

  return (
    <div
      className={`flex ${isUser ? "justify-end" : "justify-start"} mb-4`}
      data-message-id={message.id}
    >
      <div
        className={`max-w-[min(720px,92%)] rounded-lg px-3.5 py-2.5 ${
          isUser
            ? "bg-accent/20 border border-accent/30"
            : "bg-surface-2 border border-border"
        } ${isError ? "border-danger/50" : ""}`}
      >
        {!isUser && (
          <div className="text-[11px] uppercase tracking-wide text-muted mb-1">
            Assistant
            {message.status === "streaming" ? " · streaming" : ""}
          </div>
        )}
        {isUser ? (
          <p className="text-[0.9375rem] whitespace-pre-wrap">{message.body}</p>
        ) : (
          <div className="prose-chat text-foreground">
            {message.body ? (
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {message.body}
              </ReactMarkdown>
            ) : message.status === "streaming" ? (
              <span className="text-muted">…</span>
            ) : null}
          </div>
        )}
        {isError && message.errorMessage && (
          <p className="text-danger text-sm mt-2">{message.errorMessage}</p>
        )}
      </div>
    </div>
  );
}
