import { cn } from "cn";
import type { ReactNode } from "react";

const ASSISTANT_BUBBLE_MAX = "max-w-[min(88%,40rem)]";

interface AssistantMessageBubbleProps {
  children: ReactNode;
  className?: string;
  /** Avatar or other marker aligned to the left of the bubble (group chats). */
  leading?: ReactNode;
  footer?: ReactNode;
  /**
   * `prose` (default) is a soft charcoal bubble for assistant text.
   * `plain` drops the bubble entirely so rich cards can sit flush in the column.
   */
  variant?: "prose" | "plain";
}

/** Left-aligned assistant turn: low-contrast bubble, no border, no heavy shadow. */
export function AssistantMessageBubble({
  children,
  className,
  leading,
  footer,
  variant = "prose",
}: AssistantMessageBubbleProps) {
  const bubbleIndent = leading ? "ml-[2.25rem]" : undefined;

  return (
    <div className={cn("flex w-full justify-start", className)}>
      <div className={cn("flex w-full flex-col gap-1", ASSISTANT_BUBBLE_MAX)}>
        <div className="flex items-start gap-2">
          {leading ? <div className="shrink-0 pt-0.5">{leading}</div> : null}
          <div
            className={cn(
              "min-w-0 flex-1 text-[13px] leading-relaxed text-foreground",
              variant === "prose" && "rounded-xl bg-chat-assistant px-3.5 py-2.5",
            )}
          >
            {children}
          </div>
        </div>
        {footer ? <div className={bubbleIndent}>{footer}</div> : null}
      </div>
    </div>
  );
}
