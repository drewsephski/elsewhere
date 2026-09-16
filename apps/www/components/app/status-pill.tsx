import { workStatus } from "@/lib/work-events";
import { cn } from "cn";
import type { ReactNode } from "react";

export type StatusTone = "success" | "info" | "warning" | "destructive" | "neutral";

const toneClass: Record<StatusTone, string> = {
  success: "bg-success/12 text-success",
  info: "bg-info/12 text-info",
  warning: "bg-warning/12 text-warning",
  destructive: "bg-destructive/12 text-destructive",
  neutral: "bg-surface-active text-muted-foreground",
};

const dotClass: Record<StatusTone, string> = {
  success: "bg-success",
  info: "bg-info",
  warning: "bg-warning",
  destructive: "bg-destructive",
  neutral: "bg-muted-foreground",
};

interface StatusPillProps {
  tone: StatusTone;
  children: ReactNode;
  /** Pulses the dot while something is in flight. */
  live?: boolean;
  className?: string;
}

/** Compact "● Done" style pill used on work cards, rows and rails. */
export function StatusPill({ tone, children, live = false, className }: StatusPillProps) {
  return (
    <span
      className={cn(
        "inline-flex h-5 shrink-0 items-center gap-1.5 rounded-full px-2 text-[11px] font-medium leading-none whitespace-nowrap",
        toneClass[tone],
        className,
      )}
    >
      <span
        className={cn("size-1.5 rounded-full", dotClass[tone], live && "animate-pulse")}
        aria-hidden
      />
      {children}
    </span>
  );
}

export function runStatusTone(status: string): StatusTone {
  switch (status) {
    case "completed":
    case "complete":
      return "success";
    case "running":
    case "saving_results":
      return "info";
    case "queued":
    case "waiting_approval":
      return "warning";
    case "failed":
    case "interrupted":
    case "cancelled":
      return "destructive";
    default:
      return "neutral";
  }
}

export function runStatusIsLive(status: string): boolean {
  return status === "running" || status === "queued" || status === "saving_results";
}

/** Reads a run status as "Done" / "Working" / … with the matching tone. */
export function RunStatusPill({ status, className }: { status: string; className?: string }) {
  const label = status === "completed" || status === "complete" ? "Done" : workStatus(status);
  return (
    <StatusPill tone={runStatusTone(status)} live={runStatusIsLive(status)} className={className}>
      {label}
    </StatusPill>
  );
}
