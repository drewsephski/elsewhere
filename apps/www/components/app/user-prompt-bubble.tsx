import { formatMessageTime } from "@/lib/format";
import { cn } from "cn";
import type { ReactNode } from "react";

interface UserPromptBubbleProps {
  children: string;
  sentAt?: string;
  className?: string;
  extra?: ReactNode;
}

/** Right-aligned user turn: a quiet charcoal pill with a centered timestamp above. */
export function UserPromptBubble({ children, sentAt, className, extra }: UserPromptBubbleProps) {
  return (
    <div className={cn("flex w-full flex-col items-stretch gap-2", className)}>
      {sentAt ? (
        <p className="text-center text-[11px] text-muted-foreground/80">
          {formatMessageTime(sentAt)}
        </p>
      ) : null}
      <div className="flex w-full justify-end">
        <div className="max-w-[min(78%,36rem)] rounded-xl bg-chat-user px-3.5 py-2 text-[13px] leading-relaxed text-chat-user-foreground">
          {children.trim() ? (
            <p className="whitespace-pre-wrap break-words">{children}</p>
          ) : null}
          {extra}
        </div>
      </div>
    </div>
  );
}
