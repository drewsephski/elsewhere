"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { RunSummary } from "@/lib/api-types";
import { formatMessageTime } from "@/lib/format";
import { workStatus } from "@/lib/work-events";
import {
  WorkspaceDataGrid,
  WorkspaceDataGridTabs,
} from "@/components/app/workspace-data-grid";
import {
  dataGridFeatures,
  type DataGridFeatures,
} from "@/components/reui/data-grid/data-grid";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  ColumnDef,
  PaginationState,
  SortingState,
  useTable,
} from "@tanstack/react-table";
import { Archive } from "@/components/icons/lucide";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

type WorkTab = "active" | "archived";

function canArchiveRun(status: string): boolean {
  return status !== "queued" && status !== "running";
}

function statusTone(status: string): string {
  if (status === "running" || status === "queued") {
    return "text-info";
  }
  if (status === "failed" || status === "interrupted") {
    return "text-warning";
  }
  return "text-muted-foreground";
}

export function RecentRunsPanel({
  botId,
  limit = 10,
  className,
}: {
  botId?: string;
  limit?: number;
  className?: string;
}) {
  const [tab, setTab] = useState<WorkTab>("active");
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [archivingId, setArchivingId] = useState<string | null>(null);
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [sorting, setSorting] = useState<SortingState>([
    { id: "createdAt", desc: true },
  ]);

  const load = useCallback(async () => {
    try {
      const params = new URLSearchParams({ limit: String(limit) });
      if (botId) {
        params.set("bot_id", botId);
      }
      if (tab === "archived") {
        params.set("archived", "true");
      }
      const response = await cloudHostFetch(`/v1/runs?${params.toString()}`);
      if (!response.ok) {
        throw new Error("Could not load your work");
      }
      const rows: RunSummary[] = await response.json();
      setRuns(rows);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Work unavailable");
    } finally {
      setLoading(false);
    }
  }, [botId, limit, tab]);

  useEffect(() => {
    setLoading(true);
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      await load();
      if (!stopped) {
        timer = setTimeout(() => void poll(), 5000);
      }
    }
    void poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [load]);

  async function handleArchive(runId: string) {
    setArchivingId(runId);
    setError(null);
    try {
      const response = await cloudHostFetch(`/v1/runs/${runId}/archive`, {
        method: "POST",
      });
      if (!response.ok) {
        const body = (await response.json().catch(() => null)) as { error?: string } | null;
        throw new Error(body?.error ?? "Could not archive work");
      }
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not archive work");
    } finally {
      setArchivingId(null);
    }
  }

  const columns = useMemo<ColumnDef<DataGridFeatures, RunSummary>[]>(
    () => [
      {
        accessorKey: "task",
        id: "task",
        header: "Assignment",
        cell: ({ row }) => (
          <Link
            href={`/app/work/${row.original.runId}`}
            className="line-clamp-2 font-medium leading-snug text-foreground hover:underline"
          >
            {row.original.task}
          </Link>
        ),
        size: 320,
        enableSorting: false,
      },
      {
        accessorKey: "botName",
        header: "Bot",
        cell: ({ row }) => (
          <span className="text-muted-foreground">{row.original.botName}</span>
        ),
        size: 120,
      },
      {
        accessorKey: "model",
        header: "Model",
        cell: ({ row }) => (
          <span className="text-muted-foreground" title={row.original.model}>
            {row.original.model || "—"}
          </span>
        ),
        enableSorting: false,
      },
      {
        accessorKey: "createdAt",
        id: "createdAt",
        header: "Started",
        cell: ({ row }) => (
          <span className="text-muted-foreground tabular-nums">
            {formatMessageTime(row.original.createdAt)}
          </span>
        ),
        size: 100,
        sortingFn: "datetime",
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => (
          <span className={cn("font-medium", statusTone(row.original.status))}>
            {workStatus(row.original.status)}
          </span>
        ),
        size: 110,
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) =>
          tab === "active" && canArchiveRun(row.original.status) ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="text-muted-foreground hover:text-foreground"
              disabled={archivingId !== null}
              aria-label={`Archive ${row.original.task}`}
              onClick={() => void handleArchive(row.original.runId)}
            >
              <Archive className="size-4" aria-hidden />
            </Button>
          ) : null,
        size: 52,
        enableSorting: false,
      },
    ],
    [archivingId, tab],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: runs,
    pageCount: Math.ceil(runs.length / pagination.pageSize) || 1,
    getRowId: (row) => row.runId,
    state: { pagination, sorting },
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
  });

  const emptyMessage =
    tab === "archived"
      ? "Archived work will appear here."
      : "Delegate a task to a bot. Its progress and finished work will appear here.";

  return (
    <section className={cn("space-y-4", className)}>
      {error ? (
        <p className="text-xs text-destructive" role="alert">{error}</p>
      ) : null}
      <WorkspaceDataGrid
        table={table}
        recordCount={runs.length}
        loading={loading && runs.length === 0}
        emptyMessage={
          <p className="rounded-xl border border-dashed border-border bg-card px-4 py-8 text-center text-sm text-muted-foreground">
            {emptyMessage}
          </p>
        }
        toolbar={
          <WorkspaceDataGridTabs
            tabs={[
              { value: "active", label: "Active" },
              { value: "archived", label: "Archived" },
            ]}
            active={tab}
            onChange={(value) => setTab(value as WorkTab)}
            hint={
              tab === "active"
                ? "Hide finished items you no longer need."
                : "Stored on your account."
            }
          />
        }
      />
    </section>
  );
}
