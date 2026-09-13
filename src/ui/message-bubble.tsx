import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Message } from "@/lib/definitions";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { Bot, User } from "lucide-react";

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
        "mb-6 flex gap-3",
        isUser ? "flex-row-reverse" : "flex-row",
      )}
      data-message-id={message.id}
    >
      <Avatar
        className={cn(
          "size-8 shrink-0 border",
          isUser ? "border-primary/30 bg-primary/10" : "border-border bg-muted",
        )}
      >
        <AvatarFallback className="bg-transparent text-muted-foreground">
          {isUser ? <User className="size-4" /> : <Bot className="size-4" />}
        </AvatarFallback>
      </Avatar>

      <div
        className={cn(
          "min-w-0 max-w-[min(720px,85%)] space-y-1",
          isUser ? "items-end text-right" : "items-start",
        )}
      >
        <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
          <span className="font-medium uppercase tracking-wide">
            {isUser ? "You" : "Assistant"}
          </span>
          {!isUser && message.status === "streaming" && (
            <span className="text-primary">· writing</span>
          )}
        </div>

        <div
          className={cn(
            "rounded-2xl px-3.5 py-2.5 text-left text-sm leading-relaxed shadow-sm ring-1 ring-foreground/5",
            isUser
              ? "rounded-tr-md bg-primary text-primary-foreground"
              : "rounded-tl-md bg-card border border-border/80",
            isError && "border-destructive/50 bg-destructive/5",
          )}
        >
          {isUser ? (
            <p className="whitespace-pre-wrap">{message.body}</p>
          ) : isStreaming ? (
            <div className="space-y-2 py-1" aria-label="Assistant is typing">
              <Skeleton className="h-3 w-[90%]" />
              <Skeleton className="h-3 w-[70%]" />
              <Skeleton className="h-3 w-[50%]" />
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
    </div>
  );
}
