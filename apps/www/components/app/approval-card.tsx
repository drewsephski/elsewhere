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

  const resolved = status !== "pending";

  return (
    <div
      className="my-2 rounded-lg border border-amber-500/35 bg-amber-50 p-3 text-sm shadow-sm"
      role="region"
      aria-label="Action approval required"
    >
      <p className="font-medium text-foreground">Approval required</p>
      <p className="mt-1 text-foreground/85">{payload.summary}</p>
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
      {error ? (
        <p className="mt-2 text-xs text-red-700" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
