"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { formatBytes } from "@/lib/format";
import { isAssignmentSummaryKind } from "@/lib/result-preview";
import { cn } from "cn";
import { FileText } from "@/components/icons/lucide";
import { useEffect, useState } from "react";
import {
  ResultContentDialog,
  ResultOpenButton,
  resultItemTitle,
} from "@/components/app/workspace/result-content-dialog";

type ResultItem = {
  id: string;
  name: string;
  kind: string;
  size: number;
};

export function ChatResultCards({
  runId,
  className,
}: {
  runId: string;
  className?: string;
}) {
  const [items, setItems] = useState<ResultItem[]>([]);
  const [viewerOpen, setViewerOpen] = useState(false);
  const [activeItem, setActiveItem] = useState<ResultItem | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch(`/v1/runs/${runId}/results`, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          return;
        }
        const data = await response.json();
        setItems(
          (data.items ?? []).filter(
            (item: ResultItem) => !isAssignmentSummaryKind(item.kind),
          ),
        );
      })
      .catch(() => undefined);
    return () => controller.abort();
  }, [runId]);

  function handleOpen(item: ResultItem) {
    setActiveItem(item);
    setViewerOpen(true);
  }

  if (!items.length) {
    return null;
  }

  return (
    <>
      <ul className={cn("flex flex-col gap-1", className)} aria-label="Results">
        {items.map((item) => (
          <li
            key={item.id}
            className="flex items-center gap-1.5 rounded-lg bg-surface-hover px-2 py-1 transition-colors hover:bg-surface-active"
          >
            <button
              type="button"
              onClick={() => handleOpen(item)}
              className="flex min-w-0 flex-1 items-center gap-2 rounded-md py-0.5 pr-1 text-left text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <FileText className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
              <span className="min-w-0 flex-1 truncate font-medium text-foreground">
                {resultItemTitle(item.kind, item.name)}
              </span>
              <span className="shrink-0 font-mono text-[11px] text-muted-foreground">
                {formatBytes(item.size)}
              </span>
            </button>
            <ResultOpenButton
              onClick={() => handleOpen(item)}
              className="rounded-md px-1.5 py-0.5 text-[11px]"
            />
          </li>
        ))}
      </ul>

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
    </>
  );
}
