"use client";

import { useState, type KeyboardEvent } from "react";
import { Badge } from "@/components/reui/badge";
import {
  subagentHeadline,
  type SubagentActivity,
} from "@/lib/subagent-events";

function statusVariant(status: string) {
  if (status === "completed") return "success-light" as const;
  if (status === "failed" || status === "cancelled" || status === "interrupted") {
    return "destructive-light" as const;
  }
  if (status === "running") return "primary-light" as const;
  return "secondary" as const;
}

function statusLabel(status: string): string {
  return (
    {
      running: "Working",
      completed: "Finished",
      failed: "Failed",
      cancelled: "Stopped",
      interrupted: "Interrupted",
    } as Record<string, string>
  )[status] ?? status;
}

export function SubagentCard({ activity }: { activity: SubagentActivity }) {
  const [open, setOpen] = useState(activity.status !== "running");
  const details = activity.resultPreview ?? activity.error;
  const canExpand = Boolean(details);

  function handleToggle() {
    if (!canExpand) {
      return;
    }
    setOpen((value) => !value);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      handleToggle();
    }
  }

  return (
    <div
      className="flex flex-col gap-2 rounded-lg border border-border/60 bg-muted/15 px-3 py-2.5"
      data-testid="subagent-card"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 space-y-0.5">
          <p className="text-sm font-medium text-foreground">{subagentHeadline(activity)}</p>
          <p className="text-xs text-muted-foreground">
            Temporary helper for this assignment — not another Bot
          </p>
        </div>
          <Badge variant={statusVariant(activity.status)}>
            {statusLabel(activity.status)}
          </Badge>
      </div>
      {canExpand ? (
        <button
          type="button"
          className="self-start text-left text-xs text-muted-foreground underline-offset-2 hover:underline"
          onClick={handleToggle}
          onKeyDown={handleKeyDown}
          aria-expanded={open}
          tabIndex={0}
        >
          {open ? "Hide details" : "Show details"}
        </button>
      ) : null}
      {open && details ? (
        <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-background/70 p-2 text-xs text-foreground">
          {details}
        </pre>
      ) : null}
    </div>
  );
}
