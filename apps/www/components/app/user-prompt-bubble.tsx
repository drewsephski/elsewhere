import { formatMessageTime } from "@/lib/format";
import { cn } from "cn";

interface UserPromptBubbleProps {
  children: string;
  sentAt?: string;
  className?: string;
}

export function UserPromptBubble({ children, sentAt, className }: UserPromptBubbleProps) {
  return (
    <div className={cn("flex justify-end", className)}>
      <div className="max-w-[85%] rounded-3xl rounded-br-md bg-foreground px-4 py-2.5 text-sm leading-relaxed text-primary-foreground shadow-sm">
        <p className="whitespace-pre-wrap break-words">{children}</p>
        {sentAt ? (
          <p className="mt-1 text-[10px] text-white/60">{formatMessageTime(sentAt)}</p>
        ) : null}
      </div>
    </div>
  );
}
