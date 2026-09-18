"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { WorkspaceDataGrid } from "@/components/app/workspace-data-grid";
import { WorkspaceEmptyState } from "@/components/app/workspace-empty-state";
import {
  dataGridFeatures,
  type DataGridFeatures,
} from "@/components/reui/data-grid/data-grid";
import {
  ColumnDef,
  PaginationState,
  SortingState,
  useTable,
} from "@tanstack/react-table";
import {
  ResultContentDialog,
  resultItemTitle,
} from "@/components/app/workspace/result-content-dialog";
import { Download, ExternalLink, FileText } from "@/components/icons/lucide";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

type ResultItem = {
  id: string;
  runId: string;
  name: string;
  kind: string;
  size: number;
  botName: string;
  task: string;
  createdAt: string;
};
type RunResults = { items: ResultItem[]; collecting: boolean; note: string | null };

export function ResultsPanel({
  runId,
  variant = "full",
}: {
  runId?: string;
  variant?: "full" | "compact";
}) {
  const [items, setItems] = useState<ResultItem[]>([]);
  const [collecting, setCollecting] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [sorting, setSorting] = useState<SortingState>([
    { id: "createdAt", desc: true },
  ]);
  const [viewerOpen, setViewerOpen] = useState(false);
  const [activeItem, setActiveItem] = useState<ResultItem | null>(null);

  const handleOpenResult = useCallback((item: ResultItem) => {
    setActiveItem(item);
    setViewerOpen(true);
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    async function load() {
      try {
        const response = await cloudHostFetch(
          runId ? `/v1/runs/${runId}/results` : "/v1/results",
          { signal: controller.signal },
        );
        if (!response.ok) throw new Error("Could not load saved results");
        const data: RunResults = runId
          ? await response.json()
          : { items: await response.json(), collecting: false, note: null };
        if (controller.signal.aborted) return;
        setItems(data.items);
        setCollecting(data.collecting);
        setNote(data.note);
        setError(null);
        if (data.collecting || !runId) {
          timer = setTimeout(() => void load(), 5000);
        }
      } catch (err) {
        if (controller.signal.aborted) return;
        setError(err instanceof Error ? err.message : "Could not load results");
        timer = setTimeout(() => void load(), 10000);
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    }
    void load();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [runId]);

  const columns = useMemo<ColumnDef<DataGridFeatures, ResultItem>[]>(
    () => [
      {
        id: "file",
        header: "File",
        cell: ({ row }) => (
          <div className="flex min-w-0 items-start gap-2">
            <FileText className="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden />
            <div className="min-w-0">
              <p className="break-words text-sm font-medium">
                {resultItemTitle(row.original.kind, row.original.name)}
              </p>
              {!runId ? (
                <Link
                  href={`/app/work/${row.original.runId}`}
                  className="mt-1 block max-w-md truncate text-xs text-muted-foreground underline-offset-2 hover:underline"
                >
                  {row.original.task}
                </Link>
              ) : null}
            </div>
          </div>
        ),
        size: 280,
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
        accessorKey: "createdAt",
        id: "createdAt",
        header: "Saved",
        cell: ({ row }) => (
          <span className="text-muted-foreground tabular-nums">
            {new Date(row.original.createdAt).toLocaleString()}
          </span>
        ),
        size: 160,
        sortingFn: "datetime",
      },
      {
        id: "size",
        header: "Size",
        cell: ({ row }) => (
          <span className="text-muted-foreground tabular-nums">
            {Math.max(1, Math.ceil(row.original.size / 1024))} KB
          </span>
        ),
        size: 80,
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div className="flex flex-wrap items-center justify-end gap-2">
            <button
              type="button"
              onClick={() => handleOpenResult(row.original)}
              className="inline-flex items-center gap-2 rounded-lg border border-border bg-background px-2.5 py-1.5 text-sm hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <ExternalLink className="size-4" aria-hidden />
              Open
            </button>
            <a
              href={`/api/results/${row.original.id}/download`}
              download={row.original.name}
              className="inline-flex items-center gap-2 rounded-lg border border-border bg-background px-2.5 py-1.5 text-sm hover:bg-muted/40"
            >
              <Download className="size-4" aria-hidden />
              Download
            </a>
          </div>
        ),
        size: 200,
        enableSorting: false,
      },
    ],
    [runId, handleOpenResult],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: items,
    pageCount: Math.ceil(items.length / pagination.pageSize) || 1,
    getRowId: (row) => row.id,
    state: { pagination, sorting },
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
  });

  const emptyTitle = loading
    ? "Loading results"
    : collecting
      ? "Collecting results"
      : "No saved results yet";
  const emptyDescription = loading
    ? "Fetching files from your workspace…"
    : collecting
      ? "Results will be saved here when your bot finishes."
      : "Delegate work to a bot and ask for a report, draft, or file.";

  const resultDialog = (
    <ResultContentDialog
      open={viewerOpen}
      onOpenChange={setViewerOpen}
      result={
        activeItem
          ? {
              id: activeItem.id,
              title: resultItemTitle(activeItem.kind, activeItem.name),
              fileName: activeItem.name,
              size: activeItem.size,
              kind: activeItem.kind,
            }
          : null
      }
    />
  );

  if (variant === "compact") {
    if (items.length === 0) {
      if (error) {
        return (
          <p className="text-sm text-destructive" role="alert">
            {error}
          </p>
        );
      }
      if (collecting) {
        return (
          <p className="text-sm text-muted-foreground">
            Files will appear here when your bot saves them.
          </p>
        );
      }
      return null;
    }

    return (
      <section className="space-y-3">
        {error ? (
          <p className="text-sm text-destructive" role="alert">
            {error}
          </p>
        ) : null}
        {note ? <p className="text-sm text-warning">{note}</p> : null}
        <h2 className="text-sm font-medium">Files</h2>
        <ul className="divide-y divide-border rounded-lg border border-border">
          {items.map((item) => (
            <li key={item.id} className="flex items-center justify-between gap-3 px-3 py-2.5">
              <div className="flex min-w-0 items-center gap-2">
                <FileText className="size-4 shrink-0 text-muted-foreground" aria-hidden />
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">
                    {resultItemTitle(item.kind, item.name)}
                  </p>
                  <p className="text-xs text-muted-foreground tabular-nums">
                    {Math.max(1, Math.ceil(item.size / 1024))} KB
                  </p>
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-3">
                <button
                  type="button"
                  onClick={() => handleOpenResult(item)}
                  className="text-sm text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                >
                  Open
                </button>
                <a
                  href={`/api/results/${item.id}/download`}
                  download={item.name}
                  className="text-sm text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                >
                  Download
                </a>
              </div>
            </li>
          ))}
        </ul>
        {collecting ? (
          <p className="text-xs text-muted-foreground">Still collecting results…</p>
        ) : null}
        {resultDialog}
      </section>
    );
  }

  return (
    <section className="space-y-3">
      {error ? (
        <p className="text-sm text-destructive" role="alert">{error}</p>
      ) : null}
      {note ? <p className="text-sm text-warning">{note}</p> : null}
      <WorkspaceDataGrid
        table={table}
        recordCount={items.length}
        loading={loading && items.length === 0}
        emptyMessage={
          <WorkspaceEmptyState
            title={emptyTitle}
            description={emptyDescription}
            icon={<FileText aria-hidden />}
          />
        }
      />
      {items.length > 0 && collecting ? (
        <p className="text-xs text-muted-foreground">Still collecting results…</p>
      ) : null}
      {resultDialog}
    </section>
  );
}
