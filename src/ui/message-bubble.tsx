import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Message } from "@/lib/definitions";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

interface MessageBubbleProps {
  message: Message;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === "user";
  const isError = message.status === "error";
  const isStreaming = message.status === "streaming" && !message.body;

  return (
    <div
      className={cn(
        "mb-3 flex w-full",
        isUser ? "justify-end" : "justify-start",
      )}
      data-message-id={message.id}
    >
      <div
        className={cn(
          "max-w-[min(520px,88%)] px-4 py-2.5 text-sm leading-relaxed",
          isUser
            ? "rounded-[1.25rem] rounded-br-md bg-foreground text-background shadow-sm"
            : "rounded-[1.25rem] rounded-bl-md bg-[#ececef] text-foreground",
          isError && "border border-destructive/40 bg-destructive/5 text-foreground",
        )}
      >
        {isUser ? (
          <p className="whitespace-pre-wrap">{message.body}</p>
        ) : isStreaming ? (
          <div className="space-y-2 py-0.5" aria-label="Assistant is typing">
            <Skeleton className="h-3 w-[90%] bg-foreground/10" />
            <Skeleton className="h-3 w-[70%] bg-foreground/10" />
            <Skeleton className="h-3 w-[50%] bg-foreground/10" />
          </div>
        ) : (
          <div className="prose-chat text-foreground">
            {message.body ? (
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {message.body}
              </ReactMarkdown>
            ) : null}
          </div>
        )}
        {isError && message.errorMessage && (
          <p className="mt-2 text-sm text-destructive">{message.errorMessage}</p>
        )}
      </div>
    </div>
  );
}
