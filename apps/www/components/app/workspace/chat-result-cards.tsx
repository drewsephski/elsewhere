"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { formatBytes } from "@/lib/format";
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
      <div
        className={cn(
          "flex flex-col gap-1 border-t border-border/50 pt-2",
          className,
        )}
      >
        {items.map((item) => (
          <div
            key={item.id}
            className="flex items-center gap-2 rounded-lg border border-border/50 bg-muted/20 px-2 py-1"
          >
            <button
              type="button"
              onClick={() => handleOpen(item)}
              className="flex min-w-0 flex-1 items-center gap-2 text-left text-xs transition-colors rounded-md -my-0.5 py-0.5 pr-1 hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/20"
            >
              <FileText
                className="size-3.5 shrink-0 text-primary/80"
                aria-hidden
              />
              <span className="min-w-0 flex-1 truncate font-medium text-foreground/90">
                {resultItemTitle(item.kind, item.name)}
              </span>
              <span className="shrink-0 text-[11px] text-muted-foreground">
                {formatBytes(item.size)}
              </span>
            </button>
            <ResultOpenButton
              onClick={() => handleOpen(item)}
              className="rounded-md px-1.5 py-0.5 text-[11px]"
            />
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
