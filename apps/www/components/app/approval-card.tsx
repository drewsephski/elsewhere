"use client";

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
      return "Approved";
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

export function ApprovalCard({
  payload,
  externalStatus,
  onResolved,
}: ApprovalCardProps) {
  const [status, setStatus] = useState<ApprovalTerminalState>("pending");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      if (!response.ok) throw new Error(response.status === 404 ? "This request may have expired or been resolved. Refresh its work to see the latest status." : "Could not update approval. Try again.");
      const next = decisionFromApi(decision);
      setStatus(next);
      onResolved?.(next);
    } catch (err) { setError(err instanceof Error ? err.message : "Could not update approval. Try again."); }
    finally { setBusy(false); }
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
            ? "This request may have expired or been resolved. Refresh its work to see the latest status."
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
  const botName = payload.botName?.trim() || "this Bot";
  const actionLabel = (payload.policyActionLabel ?? payload.tool).toLowerCase();
  const showPersistent = payload.policyOverridable === true;
  const argumentSummary = payload.argumentSummary;

  return (
    <div
      className="my-2 rounded-lg border border-warning/30 bg-warning/10 p-3 text-sm"
      role="region"
      aria-label="Action approval required"
    >
      <p className="font-medium text-foreground">Approval required</p>
      <p className="mt-1 text-foreground/85">{payload.summary}</p>
      {payload.connectedAppName ? (
        <p className="mt-1 text-xs text-muted-foreground">
          {botName} · {payload.connectedAppName}
          {payload.connectedToolName ? ` · ${payload.connectedToolName}` : ""}
        </p>
      ) : null}
      {argumentSummary && Object.keys(argumentSummary).length > 0 ? (
        <pre className="mt-2 max-h-32 overflow-auto rounded-md bg-background/40 p-2 text-xs text-foreground/80">
          {JSON.stringify(argumentSummary, null, 2)}
        </pre>
      ) : null}
      <p className="mt-1 text-xs text-muted-foreground">Approve this action once. Your bot will wait for your decision.</p>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="sm"
          disabled={resolved || busy}
          onClick={() => void handleDecision("approve")}
        >
          Approve
        </Button>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={resolved || busy}
          onClick={() => void handleDecision("deny")}
        >
          Deny
        </Button>
        {resolved ? (
          <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {labelForStatus(status)}
          </span>
        ) : null}
      </div>
      {showPersistent && !resolved ? (
        <div className="mt-2 flex flex-col items-start gap-1">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            className="h-7 px-2 text-xs text-muted-foreground"
            disabled={busy}
            onClick={() => void handlePersistent("allow")}
          >
            Always allow {actionLabel} for {botName}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            className="h-7 px-2 text-xs text-muted-foreground"
            disabled={busy}
            onClick={() => void handlePersistent("deny")}
          >
            Always deny {actionLabel} for {botName}
          </Button>
        </div>
      ) : null}
      {error ? (
        <p className="mt-2 text-xs text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
