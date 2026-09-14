"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { formatBytes } from "@/lib/format";
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

export function ChatResultCards({ runId }: { runId: string }) {
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
        setItems(data.items ?? []);
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
      <div className="mt-2 flex flex-col gap-2">
        {items.map((item) => (
          <div
            key={item.id}
            className="flex items-center gap-3 rounded-2xl border border-border/80 bg-white px-3 py-2.5 text-sm shadow-sm"
          >
            <button
              type="button"
              onClick={() => handleOpen(item)}
              className="flex min-w-0 flex-1 items-center gap-3 text-left transition-colors rounded-xl -m-1 p-1 hover:bg-muted/30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/20"
            >
              <span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
                <FileText className="size-5" aria-hidden />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate font-medium">
                  {resultItemTitle(item.kind, item.name)}
                </span>
                <span className="text-xs text-muted-foreground">{formatBytes(item.size)}</span>
              </span>
            </button>
            <ResultOpenButton onClick={() => handleOpen(item)} />
          </div>
        ))}
      </div>

      <ResultContentDialog
        open={viewerOpen}
        onOpenChange={setViewerOpen}
        result={
          activeItem
            ? {
                id: activeItem.id,
                title: resultItemTitle(activeItem.kind, activeItem.name),
                size: activeItem.size,
              }
            : null
        }
      />
    </>
  );
}
