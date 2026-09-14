"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { formatBytes } from "@/lib/format";
import { Download, FileText } from "lucide-react";
import { useEffect, useState } from "react";

type ResultItem = {
  id: string;
  name: string;
  kind: string;
  size: number;
};

export function ChatResultCards({ runId }: { runId: string }) {
  const [items, setItems] = useState<ResultItem[]>([]);

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

  if (!items.length) {
    return null;
  }

  return (
    <div className="mt-2 flex flex-col gap-2">
      {items.map((item) => (
        <a
          key={item.id}
          href={`/api/results/${item.id}/download`}
          download={item.name}
          className="flex items-center gap-3 rounded-2xl border border-border/80 bg-white px-3 py-2.5 text-sm shadow-sm transition-colors hover:bg-muted/30"
        >
          <span className="flex size-10 items-center justify-center rounded-xl bg-primary/10 text-primary">
            <FileText className="size-5" aria-hidden />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate font-medium">
              {item.kind === "summary" ? "Assignment summary" : item.name}
            </span>
            <span className="text-xs text-muted-foreground">{formatBytes(item.size)}</span>
          </span>
          <Download className="size-4 shrink-0 text-muted-foreground" aria-hidden />
        </a>
      ))}
    </div>
  );
}
