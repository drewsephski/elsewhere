"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { WorkspaceDataGrid } from "@/components/app/workspace-data-grid";
import { WorkspaceEmptyState } from "@/components/app/workspace-empty-state";
import {
  dataGridFeatures,
  type DataGridFeatures,
} from "@/components/reui/data-grid/data-grid";
import { Button } from "@/components/ui/button";
import {
  ColumnDef,
  PaginationState,
  useTable,
} from "@tanstack/react-table";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

interface ApprovalListItem {
  approvalId: string;
  runId: string;
  toolName: string;
  toolKind: string;
  status: string;
  summary: string;
  botId?: string | null;
  botName?: string | null;
  policyOverridable?: boolean;
  policyActionLabel?: string;
}

export function ApprovalsDataGrid() {
  const [items, setItems] = useState<ApprovalListItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });

  const load = useCallback(async () => {
    try {
      const response = await cloudHostFetch("/v1/approvals?status=pending");
      if (!response.ok) {
        throw new Error("Could not load approvals");
      }
      const body = (await response.json()) as { approvals: ApprovalListItem[] };
      setItems(body.approvals);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load approvals");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const interval = setInterval(() => void load(), 5000);
    return () => clearInterval(interval);
  }, [load]);

  async function handleDecision(approvalId: string, decision: "approve" | "deny") {
    setBusyId(approvalId);
    setError(null);
    const path =
      decision === "approve"
        ? `/v1/approvals/${approvalId}/approve`
        : `/v1/approvals/${approvalId}/deny`;
    try {
      const response = await cloudHostFetch(path, { method: "POST" });
      if (!response.ok) {
        throw new Error("Could not update approval. Try again.");
      }
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update approval");
    } finally {
      setBusyId(null);
    }
  }

  async function handlePersistent(item: ApprovalListItem, kind: "allow" | "deny") {
    setBusyId(item.approvalId);
    setError(null);
    const path =
      kind === "allow"
        ? `/v1/approvals/${item.approvalId}/always-allow`
        : `/v1/approvals/${item.approvalId}/always-deny`;
    try {
      const response = await cloudHostFetch(path, { method: "POST" });
      if (!response.ok) {
        throw new Error("Could not update this Bot’s permissions. Try again.");
      }
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update this Bot’s permissions");
    } finally {
      setBusyId(null);
    }
  }

  const columns = useMemo<ColumnDef<DataGridFeatures, ApprovalListItem>[]>(
    () => [
      {
        id: "request",
        header: "Request",
        cell: ({ row }) => (
          <div className="min-w-0 py-0.5">
            <p className="line-clamp-2 text-sm font-medium leading-snug">
              {row.original.summary}
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              {row.original.toolName} · {row.original.toolKind}
            </p>
          </div>
        ),
        size: 360,
      },
      {
        id: "run",
        header: "Work",
        cell: ({ row }) => (
          <Link
            href={`/app/work/${row.original.runId}`}
            className="text-sm text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
          >
            {row.original.runId.slice(0, 8)}…
          </Link>
        ),
        size: 100,
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div className="flex flex-col items-end gap-1">
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={busyId !== null}
                onClick={() => void handleDecision(row.original.approvalId, "deny")}
              >
                Deny
              </Button>
              <Button
                type="button"
                size="sm"
                disabled={busyId !== null}
                onClick={() => void handleDecision(row.original.approvalId, "approve")}
              >
                Approve
              </Button>
            </div>
            {row.original.policyOverridable !== false ? (
              <div className="flex justify-end gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-7 px-2 text-xs text-muted-foreground"
                  disabled={busyId !== null}
                  onClick={() => void handlePersistent(row.original, "deny")}
                >
                  Always deny for {row.original.botName?.trim() || "this Bot"}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-7 px-2 text-xs text-muted-foreground"
                  disabled={busyId !== null}
                  onClick={() => void handlePersistent(row.original, "allow")}
                >
                  Always allow {row.original.policyActionLabel?.toLowerCase() || "this"} for {row.original.botName?.trim() || "this Bot"}
                </Button>
              </div>
            ) : null}
          </div>
        ),
        size: 180,
        enableSorting: false,
      },
    ],
    [busyId],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: items,
    pageCount: Math.ceil(items.length / pagination.pageSize) || 1,
    getRowId: (row) => row.approvalId,
    state: { pagination },
    onPaginationChange: setPagination,
  });

  return (
    <div className="space-y-4">
      {error ? (
        <p className="text-sm text-destructive" role="alert">{error}</p>
      ) : null}
      <WorkspaceDataGrid
        table={table}
        recordCount={items.length}
        loading={loading && items.length === 0}
        emptyMessage={
          <WorkspaceEmptyState
            title="All clear"
            description="No pending approvals. Mutating tools will show up here when your bot needs permission."
          />
        }
      />
    </div>
  );
}
