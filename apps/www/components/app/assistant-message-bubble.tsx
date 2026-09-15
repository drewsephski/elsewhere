import { cn } from "cn";
import type { ReactNode } from "react";

const ASSISTANT_BUBBLE_MAX = "max-w-[min(85%,42rem)]";

interface AssistantMessageBubbleProps {
  children: ReactNode;
  className?: string;
  /** Avatar or other marker aligned to the left of the bubble (group chats). */
  leading?: ReactNode;
  footer?: ReactNode;
}

export function AssistantMessageBubble({
  children,
  className,
  leading,
  footer,
}: AssistantMessageBubbleProps) {
  const bubbleIndent = leading ? "ml-[2.625rem]" : undefined;

  return (
    <div className={cn("flex w-full justify-start", className)}>
      <div className={cn("flex w-full flex-col gap-1", ASSISTANT_BUBBLE_MAX)}>
        <div className="flex items-start gap-2.5">
          {leading ? <div className="shrink-0">{leading}</div> : null}
          <div
            className={cn(
              "min-w-0 rounded-3xl border border-border/80 bg-white px-4 py-3 text-sm shadow-sm",
              leading ? "flex-1 rounded-tl-md" : "w-full rounded-bl-md",
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
