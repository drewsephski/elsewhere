"use client";

import { ApprovalCard, type ApprovalRequestedPayload } from "@/components/app/approval-card";
import { cloudHostFetch } from "@/lib/cloud-api";
import { useCallback, useEffect, useState } from "react";

interface ApprovalListItem {
  approvalId: string;
  runId: string;
  toolName: string;
  toolKind: string;
  status: string;
  summary: string;
}

export default function ApprovalsPage() {
  const [items, setItems] = useState<ApprovalListItem[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const response = await cloudHostFetch("/v1/approvals?status=pending");
    if (!response.ok) {
      setError("Could not load approvals");
      return;
    }
    const body = (await response.json()) as { approvals: ApprovalListItem[] };
    setItems(body.approvals);
    setError(null);
  }, []);

  useEffect(() => {
    void load();
    const interval = setInterval(() => void load(), 5000);
    return () => clearInterval(interval);
  }, [load]);

  return (
    <div>
      <h1 className="text-2xl font-semibold tracking-tight">Pending approvals</h1>
      <p className="mt-2 text-sm text-muted-foreground">
        Mutating computer tools blocked until you approve or deny.
      </p>
      {error ? (
        <p className="mt-4 text-sm text-red-700" role="alert">
          {error}
        </p>
      ) : null}
      <div className="mt-6 flex flex-col gap-3">
        {items.length === 0 ? (
          <p className="text-sm text-muted-foreground">No pending approvals.</p>
        ) : (
          items.map((item) => (
            <div key={item.approvalId}>
              <p className="mb-1 text-xs text-muted-foreground">Run {item.runId.slice(0, 8)}…</p>
              <ApprovalCard
                payload={{
                  approvalId: item.approvalId,
                  tool: item.toolName,
                  operationKind: item.toolKind,
                  summary: item.summary,
                }}
                onResolved={() => void load()}
              />
            </div>
          ))
        )}
      </div>
    </div>
  );
}
