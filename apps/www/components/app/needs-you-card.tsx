"use client";

import { cn } from "cn";
import type { ReactNode } from "react";

export type NeedsYouTone = "pending" | "resolved" | "neutral";

interface NeedsYouCardProps {
  title: string;
  /** Short explanation of why work paused. */
  reason: string;
  /** What the bot needs (action target, question text, approval summary). */
  detail?: ReactNode;
  /** Reassurance that work continues after the user acts. */
  continuation?: string;
  actions?: ReactNode;
  className?: string;
  tone?: NeedsYouTone;
}

/**
 * Shared shell for inline “needs you” moments in chat (approvals, choices, browser sign-in).
 */
const toneStyles: Record<NeedsYouTone, string> = {
  pending: "border-warning/30 bg-warning/10",
  resolved: "border-border/60 bg-muted/30",
  neutral: "border-border/60 bg-muted/20",
};

export function NeedsYouCard({
  title,
  reason,
  detail,
  continuation = "Your bot continues after you respond.",
  actions,
  className,
  tone = "pending",
}: NeedsYouCardProps) {
  const showContinuation = tone !== "resolved" && continuation;
  return (
    <div
      className={cn(
        "my-2 rounded-lg border p-3 text-sm",
        toneStyles[tone],
        className,
      )}
      role="region"
      aria-label={title}
    >
      <p className="font-medium text-foreground">{title}</p>
      <p className="mt-1 text-[13px] text-foreground/90">{reason}</p>
      {detail ? <div className="mt-1.5 text-[13px] text-foreground/85">{detail}</div> : null}
      {showContinuation ? (
        <p className="mt-2 text-xs text-muted-foreground">{continuation}</p>
      ) : null}
      {actions ? <div className="mt-3 flex flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  );
}
