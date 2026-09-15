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
    } as Record<string, string>
  )[status] ?? status;
}

function statusVariant(status: string) {
  if (status === "completed") return "success-light" as const;
  if (status === "failed" || status === "cancelled") return "destructive-light" as const;
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
  return (
    <div
      className="flex flex-col gap-3 rounded-lg border border-border/60 bg-muted/20 p-4 sm:flex-row sm:items-center sm:justify-between"
      data-testid="delegation-card"
    >
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
        <Badge variant={statusVariant(delegation.status)}>
          {delegationStatusLabel(delegation.status)}
        </Badge>
        {targetRunId ? (
          <Link
            href={`/app/work/${targetRunId}`}
            className="inline-flex h-8 items-center justify-center rounded-md border border-input bg-background px-3 text-sm font-medium hover:bg-muted"
          >
            View {delegation.targetBotName}&apos;s work
          </Link>
        ) : null}
        {resumeRunId ? (
          <Link
            href={`/app/work/${resumeRunId}`}
            className="inline-flex h-8 items-center justify-center rounded-md border border-input bg-background px-3 text-sm font-medium hover:bg-muted"
          >
            View {displaySourceName}&apos;s follow-up
          </Link>
        ) : null}
        {delegation.returnPolicy ? (
          <span className="text-xs text-muted-foreground">
            Return: {delegation.returnPolicy === "resume_source" ? "resume source" : "none"}
          </span>
        ) : null}
        {delegation.resumeStatus === "skipped" && delegation.resumeError ? (
          <span className="text-xs text-destructive">{delegation.resumeError}</span>
        ) : null}
      </div>
    </div>
  );
}
