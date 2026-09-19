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
  /** Tighter padding and type for inline chat moments (e.g. approvals). */
  compact?: boolean;
  leading?: ReactNode;
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
  compact = false,
  leading,
}: NeedsYouCardProps) {
  const showContinuation = tone !== "resolved" && continuation;
  return (
    <div
      className={cn(
        compact ? "my-1 rounded-md border px-2.5 py-2 text-xs" : "my-2 rounded-lg border p-3 text-sm",
        toneStyles[tone],
        className,
      )}
      role="region"
      aria-label={title}
    >
      <div className={cn("flex items-start", leading ? "gap-2.5" : null)}>
        {leading ? <div className="mt-0.5 shrink-0 text-foreground">{leading}</div> : null}
        <div className="min-w-0 flex-1">
          <p className={cn("text-foreground", compact ? "text-xs font-semibold" : "font-medium")}>
            {title}
          </p>
          <p
            className={cn(
              "text-foreground/90",
              compact ? "mt-0.5 text-xs leading-snug" : "mt-1 text-[13px]",
            )}
          >
            {reason}
          </p>
          {detail ? (
            <div
              className={cn(
                "text-foreground/85",
                compact ? "mt-0.5 text-[11px]" : "mt-1.5 text-[13px]",
              )}
            >
              {detail}
            </div>
          ) : null}
          {showContinuation ? (
            <p className={cn("text-muted-foreground", compact ? "mt-1 text-[10px]" : "mt-2 text-xs")}>
              {continuation}
            </p>
          ) : null}
          {actions ? (
            <div
              className={cn(
                "flex flex-col flex-wrap items-stretch",
                compact ? "mt-2 gap-1.5" : "mt-3 gap-2",
              )}
            >
              {actions}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
