import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Message } from "@desktop/lib/definitions";
import { Skeleton } from "@desktop/components/ui/skeleton";
import { cn } from "@desktop/lib/utils";

interface MessageBubbleProps {
  message: Message;
}

function formatTimelineLabel(message: Message): string | null {
  if (message.kind === "tool_call") {
    try {
      const data = JSON.parse(message.body) as {
        tool?: string;
        arguments?: { path?: string; command?: string };
      };
      const tool = data.tool ?? "tool";
      const path = data.arguments?.path;
      const command = data.arguments?.command;
      if (tool === "workspace_write" && path) {
        return `Writing ${path}`;
      }
      if (tool === "workspace_read" && path) {
        return `Reading ${path}`;
      }
      if (tool === "workspace_list" && path) {
        return `Listing ${path}`;
      }
      if (tool === "workspace_exec" && command) {
        return `Running ${command}`;
      }
      return tool;
    } catch {
      return "Tool call";
    }
  }
  if (message.kind === "tool_result") {
    try {
      const data = JSON.parse(message.body) as { tool?: string; ok?: boolean };
      const tool = data.tool ?? "tool";
      return data.ok === false ? `${tool} failed` : `${tool} completed`;
    } catch {
      return "Tool result";
    }
  }
  if (message.kind === "agent_status") {
    try {
      const data = JSON.parse(message.body) as { status?: string };
      if (data.status === "running") {
        return "Agent working…";
      }
      if (data.status === "completed") {
        return "Agent finished";
      }
      if (data.status === "failed") {
        return "Agent failed";
      }
    } catch {
      return null;
    }
  }
  return null;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const timelineLabel = formatTimelineLabel(message);
  if (timelineLabel) {
    return (
      <div
        className="mb-2 flex w-full justify-center"
        data-message-id={message.id}
      >
        <div className="rounded-full border border-border/60 bg-muted/40 px-3 py-1 text-xs text-muted-foreground">
          {timelineLabel}
        </div>
      </div>
    );
  }

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
