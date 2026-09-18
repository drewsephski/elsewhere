import type { Message } from "@desktop/lib/definitions";
import { AssistantMessageBubble } from "@/components/app/assistant-message-bubble";
import { MarkdownContent } from "@/components/app/markdown-content";
import { UserPromptBubble } from "@/components/app/user-prompt-bubble";
import { Skeleton } from "@desktop/components/ui/skeleton";

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
      <div className="mb-2 flex w-full justify-center" data-message-id={message.id}>
        <p className="text-center text-[11px] text-muted-foreground">{timelineLabel}</p>
      </div>
    );
  }

  const isUser = message.role === "user";
  const isError = message.status === "error";
  const isStreaming = message.status === "streaming" && !message.body;

  if (isUser) {
    return (
      <div className="space-y-2.5" data-message-id={message.id}>
        <UserPromptBubble
          sentAt={new Date(message.createdAt).toISOString()}
        >
          {message.body}
        </UserPromptBubble>
        {isError && message.errorMessage ? (
          <p className="text-center text-[11px] text-destructive" role="alert">
            {message.errorMessage}
          </p>
        ) : null}
      </div>
    );
  }

  return (
    <div className="space-y-2.5" data-message-id={message.id}>
      <AssistantMessageBubble>
        {isStreaming ? (
          <div className="space-y-2 py-0.5" aria-label="Assistant is typing">
            <Skeleton className="h-3 w-[90%] bg-foreground/10" />
            <Skeleton className="h-3 w-[70%] bg-foreground/10" />
            <Skeleton className="h-3 w-[50%] bg-foreground/10" />
          </div>
        ) : message.body ? (
          <MarkdownContent text={message.body} />
        ) : null}
        {isError && message.errorMessage ? (
          <p className="mt-2 text-[11px] text-destructive">{message.errorMessage}</p>
        ) : null}
      </AssistantMessageBubble>
    </div>
  );
}
