"use client";

import { Button } from "@/components/ui/button";
import { NeedsYouCard } from "@/components/app/needs-you-card";
import {
  runRecoveryGuide,
  type RunRecoveryAction,
} from "@/lib/run-recovery";
import Link from "next/link";

interface RunFailureCardProps {
  runId: string;
  status: string;
  errorCode?: string | null;
  onOpenSettings?: (section: "chatgpt" | "general" | "advanced") => void;
  onFocusComposer?: () => void;
  /** Prefill composer with the original task; must not auto-send. */
  onRetryMessage?: () => void;
}

function RecoveryActionButton({
  action,
  onOpenSettings,
  onRetryMessage,
}: {
  action: RunRecoveryAction;
  onOpenSettings?: (section: "chatgpt" | "general" | "advanced") => void;
  onRetryMessage?: () => void;
}) {
  if (action.kind === "view_details") {
    return (
      <Link
        href={action.href}
        className="inline-flex h-8 items-center justify-center rounded-md border border-border bg-transparent px-3 text-sm font-medium hover:bg-surface-hover"
      >
        {action.label}
      </Link>
    );
  }
  if (action.kind === "connect_chatgpt" || action.kind === "open_settings") {
    return (
      <Button
        type="button"
        size="sm"
        onClick={() => onOpenSettings?.(action.settingsSection)}
      >
        {action.label}
      </Button>
    );
  }
  if (action.kind === "retry_message") {
    return (
      <Button type="button" size="sm" variant="outline" onClick={onRetryMessage}>
        {action.label}
      </Button>
    );
  }
  return null;
}

export function RunFailureCard({
  runId,
  status,
  errorCode,
  onOpenSettings,
  onRetryMessage,
}: RunFailureCardProps) {
  const guide = runRecoveryGuide(status, errorCode, runId);
  if (!guide) {
    return null;
  }

  return (
    <NeedsYouCard
      tone="neutral"
      title={guide.title}
      reason={guide.reason}
      continuation={guide.continuation}
      actions={guide.actions.map((action) => (
        <RecoveryActionButton
          key={`${action.kind}-${action.label}`}
          action={action}
          onOpenSettings={onOpenSettings}
          onRetryMessage={onRetryMessage}
        />
      ))}
    />
  );
}
