"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { useState } from "react";
import { Button } from "@/components/ui/button";

export interface ApprovalRequestedPayload {
  approvalId: string;
  tool: string;
  operationKind: string;
  summary: string;
  waitingForApproval?: boolean;
}

interface ApprovalCardProps {
  payload: ApprovalRequestedPayload;
  onResolved?: (decision: "approved" | "denied") => void;
}

export function ApprovalCard({ payload, onResolved }: ApprovalCardProps) {
  const [status, setStatus] = useState<"pending" | "approved" | "denied" | "resolved">("pending");
  const [error, setError] = useState<string | null>(null);

  async function handleDecision(decision: "approve" | "deny") {
    if (status !== "pending") {
      return;
    }
    setError(null);
    const path =
      decision === "approve"
        ? `/v1/approvals/${payload.approvalId}/approve`
        : `/v1/approvals/${payload.approvalId}/deny`;
    const response = await cloudHostFetch(path, { method: "POST" });
    if (!response.ok) {
      setError("Could not update approval");
      return;
    }
    const next = decision === "approve" ? "approved" : "denied";
    setStatus(next);
    onResolved?.(next);
  }

  const resolved = status !== "pending";

  return (
    <div
      className="my-2 border border-amber-600/40 bg-amber-50/80 p-3 text-sm dark:border-amber-500/30 dark:bg-amber-950/30"
      role="region"
      aria-label="Tool approval required"
    >
      <p className="font-medium text-brand-dark dark:text-brand-cream">Approval required</p>
      <p className="mt-1 text-brand-dark/80 dark:text-brand-cream/80">{payload.summary}</p>
      <p className="mt-1 font-mono text-[11px] text-brand-dark/50">{payload.tool}</p>
      <div className="mt-3 flex flex-wrap gap-2">
        <Button
          type="button"
          size="sm"
          disabled={resolved}
          onClick={() => void handleDecision("approve")}
        >
          Approve
        </Button>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={resolved}
          onClick={() => void handleDecision("deny")}
        >
          Deny
        </Button>
        {resolved ? (
          <span className="text-xs uppercase tracking-wide text-brand-dark/50">
            {status === "approved" ? "Approved" : "Denied"}
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
