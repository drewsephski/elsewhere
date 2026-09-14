"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { useEffect, useState } from "react";

export function RunAssistantSnippet({
  runId,
  fallbackText,
}: {
  runId: string;
  fallbackText?: string | null;
}) {
  const [text, setText] = useState<string | null>(
    fallbackText?.trim() ? fallbackText.trim() : null,
  );

  useEffect(() => {
    if (fallbackText?.trim()) {
      setText(fallbackText.trim());
    }
  }, [fallbackText]);

  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch(`/v1/runs/${runId}`, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          return;
        }
        const detail = await response.json();
        if (typeof detail.assistantResult === "string" && detail.assistantResult.trim()) {
          setText(detail.assistantResult);
        }
      })
      .catch(() => undefined);
    return () => controller.abort();
  }, [runId]);

  if (!text) {
    return null;
  }

  return (
    <p className="mt-2 line-clamp-6 whitespace-pre-wrap break-words leading-relaxed text-foreground/90">
      {text}
    </p>
  );
}
