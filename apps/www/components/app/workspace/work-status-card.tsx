"use client";

import { RunStatusPill } from "@/components/app/status-pill";
import { ArrowUpRight, Monitor } from "@/components/icons/lucide";
import { buttonVariants } from "@/components/ui/button";
import type { RunSummary } from "@/lib/api-types";
import { formatMessageTime } from "@/lib/format";
import { cn } from "cn";
import Link from "next/link";
import type { ReactNode } from "react";

interface WorkStatusCardProps {
  run: Pick<
    RunSummary,
    "runId" | "task" | "status" | "botName" | "model" | "createdAt" | "startedAt" | "finishedAt"
  >;
  /** Overrides the derived first-line title. */
  title?: string;
  /** Overrides the status the pill reads (e.g. live stream status). */
  status?: string;
  /** Extra meta shown in the second line (e.g. "3 results"). */
  meta?: ReactNode;
  /** Quiet secondary actions rendered next to the primary CTA. */
  actions?: ReactNode;
  /** Body content between the meta rows and the action row (results, delegations…). */
  children?: ReactNode;
  className?: string;
}

const TITLE_MAX = 72;

/** First line of the task, trimmed to a card-friendly title. */
export function workCardTitle(task: string | null | undefined): string {
  const firstLine = (task ?? "").split("\n").find((line) => line.trim())?.trim() ?? "";
  if (!firstLine) {
    return "Work";
  }
  if (firstLine.length <= TITLE_MAX) {
    return firstLine;
  }
  return `${firstLine.slice(0, TITLE_MAX - 1).trimEnd()}…`;
}

function formatDuration(startedAt: string | null, finishedAt: string | null): string | null {
  if (!startedAt || !finishedAt) {
    return null;
  }
  const ms = new Date(finishedAt).getTime() - new Date(startedAt).getTime();
  if (!Number.isFinite(ms) || ms < 0) {
    return null;
  }
  const totalSeconds = Math.round(ms / 1000);
  if (totalSeconds < 60) {
    return `${totalSeconds}s`;
  }
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) {
    return seconds ? `${minutes}m ${seconds}s` : `${minutes}m`;
  }
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

/**
 * Rich work card: bold title, status pill, mono meta rows, results, and a
 * solid primary CTA with quieter secondary actions.
 */
export function WorkStatusCard({
  run,
  title,
  status,
  meta,
  actions,
  children,
  className,
}: WorkStatusCardProps) {
  const effectiveStatus = status ?? run.status;
  const duration = formatDuration(run.startedAt, run.finishedAt);
  const isDone = effectiveStatus === "completed" || effectiveStatus === "complete";
  const timeLabel = isDone && run.finishedAt
    ? `Finished ${formatMessageTime(run.finishedAt)}`
    : `Started ${formatMessageTime(run.startedAt ?? run.createdAt)}`;

  return (
    <section
      className={cn(
        "w-full max-w-[min(100%,40rem)] rounded-xl border border-border bg-card px-3.5 py-3 text-[13px]",
        className,
      )}
      aria-label="Work status"
    >
      <div className="flex items-start justify-between gap-3">
        <p className="min-w-0 flex-1 truncate font-semibold leading-5 text-foreground">
          {title ?? workCardTitle(run.task)}
        </p>
        <RunStatusPill status={effectiveStatus} />
      </div>

      <div className="mt-1.5 space-y-1 text-xs text-muted-foreground">
        <div className="flex min-w-0 items-center gap-1.5">
          <Monitor className="size-3.5 shrink-0 opacity-80" aria-hidden />
          <span className="truncate">
            {run.botName}
            {run.model ? (
              <>
                {" · "}
                <span className="font-mono text-[11px]">{run.model}</span>
              </>
            ) : null}
          </span>
        </div>
        <p className="flex flex-wrap items-center gap-x-1.5 gap-y-0.5">
          <span>{timeLabel}</span>
          {duration ? <span>· {duration}</span> : null}
          {meta ? <span>· {meta}</span> : null}
        </p>
      </div>

      {children ? <div className="mt-2.5">{children}</div> : null}

      <div className="mt-3 flex flex-wrap items-center gap-1.5">
        <Link
          href={`/app/work/${run.runId}`}
          className={cn(
            buttonVariants({ size: "sm" }),
            "h-7 gap-1 rounded-lg px-2.5 text-xs font-medium",
          )}
        >
          View work
          <ArrowUpRight className="size-3.5" aria-hidden />
        </Link>
        {actions}
      </div>
    </section>
  );
}
