"use client";

import Link from "next/link";
import type { DelegationSummary } from "@/lib/api-types";
import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { Badge } from "@/components/reui/badge";

function delegationStatusLabel(status: string): string {
  return (
    {
      queued: "Queued",
      running: "Working",
      completed: "Finished",
      failed: "Failed",
      cancelled: "Stopped",
      interrupted: "Interrupted",
    } as Record<string, string>
  )[status] ?? status;
}

function resumeStatusLabel(status: string): string {
  return (
    {
      queued: "Queued",
      running: "Working",
      completed: "Finished",
      failed: "Failed",
      skipped: "Skipped",
    } as Record<string, string>
  )[status] ?? status;
}

function statusVariant(status: string) {
  if (status === "completed") return "success-light" as const;
  if (status === "failed" || status === "cancelled" || status === "interrupted")
    return "destructive-light" as const;
  if (status === "running") return "primary-light" as const;
  return "secondary" as const;
}

interface DelegationCardProps {
  delegation: DelegationSummary;
  sourceBotName?: string;
}

export function DelegationCard({ delegation, sourceBotName }: DelegationCardProps) {
  const targetRunId = delegation.targetRunId;
  const resumeRunId = delegation.sourceResumeRunId;
  const displaySourceName = sourceBotName ?? delegation.sourceBotName;
  const targetLabel =
    delegation.targetRunStatus && delegationStatusLabel(delegation.targetRunStatus) !== delegation.status
      ? delegationStatusLabel(delegation.targetRunStatus)
      : delegationStatusLabel(delegation.status);
  const resumeStatus = delegation.resumeStatus;
  const showResumeChain =
    delegation.returnPolicy === "resume_source" &&
    (resumeStatus != null || resumeRunId != null);

  return (
    <div
      className="flex flex-col gap-3 rounded-lg border border-border/60 bg-muted/20 p-4"
      data-testid="delegation-card"
    >
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex min-w-0 items-start gap-3">
          <BotCreatureAvatar
            avatarId={delegation.targetBotAvatarId ?? DEFAULT_BOT_AVATAR_ID}
            name={delegation.targetBotName}
            size="sm"
          />
          <div className="min-w-0 space-y-1">
            <p className="font-medium text-foreground">{delegation.targetBotName}</p>
            <p className="text-sm text-muted-foreground">
              Handoff from {delegation.sourceBotName}
            </p>
            <p className="line-clamp-2 text-sm text-muted-foreground">{delegation.instruction}</p>
          </div>
        </div>
        <div className="flex shrink-0 flex-wrap items-center gap-2">
          <Badge variant={statusVariant(delegation.status)}>{targetLabel}</Badge>
          {targetRunId ? (
            <Link
              href={`/app/work/${targetRunId}`}
              className="inline-flex h-8 items-center justify-center rounded-md border border-input bg-background px-3 text-sm font-medium hover:bg-muted"
            >
              View {delegation.targetBotName}&apos;s work
            </Link>
          ) : null}
        </div>
      </div>

      {showResumeChain ? (
        <div className="flex flex-col gap-2 border-t border-border/50 pt-3 pl-1">
          <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Returned to {displaySourceName}
          </p>
          {resumeStatus === "skipped" ? (
            <p className="text-sm text-destructive">
              {delegation.resumeError ?? "Source continuation was skipped"}
            </p>
          ) : resumeStatus === "queued" || resumeStatus === "running" ? (
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={statusVariant(resumeStatus ?? "queued")}>
                {displaySourceName} {resumeStatusLabel(resumeStatus ?? "queued").toLowerCase()}…
              </Badge>
              {resumeRunId ? (
                <Link
                  href={`/app/work/${resumeRunId}`}
                  className="text-sm font-medium text-primary hover:underline"
                >
                  View follow-up
                </Link>
              ) : null}
            </div>
          ) : resumeStatus === "completed" ? (
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="success-light">
                {displaySourceName} followed up · Finished
              </Badge>
              {resumeRunId ? (
                <Link
                  href={`/app/work/${resumeRunId}`}
                  className="inline-flex h-8 items-center justify-center rounded-md border border-input bg-background px-3 text-sm font-medium hover:bg-muted"
                >
                  View follow-up
                </Link>
              ) : null}
            </div>
          ) : resumeStatus === "failed" ? (
            <div className="space-y-1">
              <Badge variant="destructive-light">
                {displaySourceName} resumed to handle the interruption
              </Badge>
              {delegation.resumeError ? (
                <p className="text-xs text-muted-foreground">{delegation.resumeError}</p>
              ) : null}
              {resumeRunId ? (
                <Link
                  href={`/app/work/${resumeRunId}`}
                  className="text-sm font-medium text-primary hover:underline"
                >
                  View follow-up
                </Link>
              ) : null}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">Preparing source continuation…</p>
          )}
        </div>
      ) : null}
    </div>
  );
}
