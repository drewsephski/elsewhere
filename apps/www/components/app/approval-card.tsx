"use client";

import { NeedsYouCard } from "@/components/app/needs-you-card";
import { cloudHostFetch } from "@/lib/cloud-api";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";

export type ApprovalTerminalState =
  | "pending"
  | "approved"
  | "denied"
  | "cancelled"
  | "expired";

export interface ApprovalRequestedPayload {
  approvalId: string;
  tool: string;
  operationKind: string;
  summary: string;
  waitingForApproval?: boolean;
  botId?: string;
  botName?: string;
  policyOverridable?: boolean;
  policyActionLabel?: string;
  connectedAppName?: string;
  connectedToolName?: string;
  argumentSummary?: Record<string, unknown>;
}

interface ApprovalCardProps {
  payload: ApprovalRequestedPayload;
  externalStatus?: ApprovalTerminalState;
  onResolved?: (decision: ApprovalTerminalState) => void;
}

function decisionFromApi(decision: "approve" | "deny"): ApprovalTerminalState {
  return decision === "approve" ? "approved" : "denied";
}

function labelForStatus(status: ApprovalTerminalState): string {
  switch (status) {
    case "approved":
      return "Allowed";
    case "denied":
      return "Denied";
    case "cancelled":
      return "Cancelled";
    case "expired":
      return "Expired";
    default:
      return "Pending";
  }
}

function humanTarget(payload: ApprovalRequestedPayload): string | null {
  if (payload.connectedAppName) {
    const tool = payload.connectedToolName ? ` · ${payload.connectedToolName}` : "";
    return `${payload.connectedAppName}${tool}`;
  }
  if (payload.policyActionLabel) {
    return payload.policyActionLabel;
  }
  return null;
}

export function ApprovalCard({
  payload,
  externalStatus,
  onResolved,
}: ApprovalCardProps) {
  const [status, setStatus] = useState<ApprovalTerminalState>("pending");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);

  useEffect(() => {
    if (!externalStatus || externalStatus === "pending") {
      return;
    }
    setStatus(externalStatus);
  }, [externalStatus]);

  async function handleDecision(decision: "approve" | "deny") {
    if (status !== "pending" || busy) {
      return;
    }
    setError(null);
    const path =
      decision === "approve"
        ? `/v1/approvals/${payload.approvalId}/approve`
        : `/v1/approvals/${payload.approvalId}/deny`;
    setBusy(true);
    try {
      const response = await cloudHostFetch(path, { method: "POST" });
      if (!response.ok)
        throw new Error(
          response.status === 404
            ? "This request may have expired or been resolved. Refresh to see the latest status."
            : "Could not update approval. Try again.",
        );
      const next = decisionFromApi(decision);
      setStatus(next);
      onResolved?.(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update approval. Try again.");
    } finally {
      setBusy(false);
    }
  }

  async function handlePersistent(kind: "allow" | "deny") {
    if (status !== "pending" || busy) {
      return;
    }
    setError(null);
    const path =
      kind === "allow"
        ? `/v1/approvals/${payload.approvalId}/always-allow`
        : `/v1/approvals/${payload.approvalId}/always-deny`;
    setBusy(true);
    try {
      const response = await cloudHostFetch(path, { method: "POST" });
      if (!response.ok) {
        throw new Error(
          response.status === 404
            ? "This request may have expired or been resolved. Refresh to see the latest status."
            : "Could not update this Bot’s permissions. Try again.",
        );
      }
      const next: ApprovalTerminalState = kind === "allow" ? "approved" : "denied";
      setStatus(next);
      onResolved?.(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update this Bot’s permissions. Try again.");
    } finally {
      setBusy(false);
    }
  }

  const resolved = status !== "pending";
  const resolvedTitle =
    status === "approved"
      ? `Allowed · ${payload.summary}`
      : status === "denied"
        ? `Denied · ${payload.summary}`
        : `${labelForStatus(status)} · ${payload.summary}`;
  const botName = payload.botName?.trim() || "this Bot";
  const actionLabel = (payload.policyActionLabel ?? payload.tool).toLowerCase();
  const showPersistent = payload.policyOverridable === true;
  const argumentSummary = payload.argumentSummary;
  const hasDetails =
    argumentSummary && Object.keys(argumentSummary).length > 0;
  const target = humanTarget(payload);
  const summaryLine = target ? `${payload.summary} · ${target}` : payload.summary;
  const isRoutineApproval = payload.tool.startsWith("routine_");
  const isSkillApproval = payload.tool.startsWith("skill_");
  const pendingTitle = isRoutineApproval
    ? "Allow this routine?"
    : isSkillApproval
      ? payload.tool === "skill_save_recent_work"
        ? "Allow this skill?"
        : payload.tool === "skill_detach"
          ? "Remove this skill?"
          : "Allow this skill change?"
      : "Allow this action?";

  if (resolved) {
    return (
      <NeedsYouCard
        compact
        tone="resolved"
        title={resolvedTitle}
        reason={target ? target : "Recorded in this conversation."}
        actions={
          hasDetails ? (
            <div className="w-full pt-1">
              <button
                type="button"
                className="text-xs font-medium text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
                onClick={() => setDetailsOpen((open) => !open)}
                aria-expanded={detailsOpen}
              >
                {detailsOpen ? "Hide details" : "Show details"}
              </button>
              {detailsOpen ? (
                <pre className="mt-2 max-h-32 overflow-auto rounded-md bg-background/40 p-2 text-xs text-foreground/80">
                  {JSON.stringify(argumentSummary, null, 2)}
                </pre>
              ) : null}
            </div>
          ) : null
        }
      />
    );
  }

  return (
    <NeedsYouCard
      compact
      tone="pending"
      title={pendingTitle}
      reason={summaryLine}
      continuation=""
      actions={
        <>
          <div className="flex w-full flex-wrap items-center gap-1.5">
            <Button
              type="button"
              size="sm"
              className="min-h-8 flex-1 sm:flex-none"
              disabled={busy}
              onClick={() => void handleDecision("approve")}
            >
              Allow
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="min-h-8 flex-1 sm:flex-none"
              disabled={busy}
              onClick={() => void handleDecision("deny")}
            >
              Deny
            </Button>
          </div>
          {showPersistent && !resolved ? (
            <div className="flex w-full flex-wrap gap-1.5">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                className="h-8 min-h-8 flex-1 text-xs font-medium"
                disabled={busy}
                onClick={() => void handlePersistent("allow")}
                aria-label={`Always allow ${actionLabel} for ${botName}`}
              >
                Always allow
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                className="h-8 min-h-8 flex-1 border-border/80 text-xs font-medium"
                disabled={busy}
                onClick={() => void handlePersistent("deny")}
                aria-label={`Always deny ${actionLabel} for ${botName}`}
              >
                Always deny
              </Button>
            </div>
          ) : null}
          {hasDetails ? (
            <div className="w-full pt-1">
              <button
                type="button"
                className="text-xs font-medium text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
                onClick={() => setDetailsOpen((open) => !open)}
                aria-expanded={detailsOpen}
              >
                {detailsOpen ? "Hide details" : "Show details"}
              </button>
              {detailsOpen ? (
                <pre className="mt-2 max-h-32 overflow-auto rounded-md bg-background/40 p-2 text-xs text-foreground/80">
                  {JSON.stringify(argumentSummary, null, 2)}
                </pre>
              ) : null}
            </div>
          ) : null}
          {error ? (
            <p className="w-full text-xs text-destructive" role="alert">
              {error}
            </p>
          ) : null}
        </>
      }
    />
  );
}
