"use client";

import { ArrowUpRight } from "@/components/icons/lucide";
import type { RunSummary } from "@/lib/api-types";
import { cn } from "cn";
import Link from "next/link";
import type { ReactNode } from "react";

interface WorkStatusCardProps {
  run: Pick<RunSummary, "runId">;
  /** Quiet secondary actions rendered next to the primary link. */
  actions?: ReactNode;
  /** Body content below the link (results, delegations…). */
  children?: ReactNode;
  className?: string;
}

/**
 * Quiet follow-up under a bot reply: a muted "View work" link with the
 * animated arrow, optional results, and archive/delete.
 */
export function WorkStatusCard({
  run,
  actions,
  children,
  className,
}: WorkStatusCardProps) {
  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <div className="flex flex-wrap items-center gap-1">
        <Link
          href={`/app/work/${run.runId}`}
          className="inline-flex items-center gap-1 rounded-md py-0.5 text-[12px] text-muted-foreground/70 transition-colors hover:text-foreground"
        >
          Details
          <ArrowUpRight className="size-3.5" aria-hidden />
        </Link>
        {actions}
      </div>
      {children ? <div>{children}</div> : null}
    </div>
  );
}
