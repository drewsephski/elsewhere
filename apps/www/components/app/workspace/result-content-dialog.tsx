"use client";

import { formatBytes } from "@/lib/format";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { ExternalLink, FileText } from "@/components/icons/lucide";
import { MarkdownContent } from "@/components/app/markdown-content";
import {
  isMarkdownResultFile,
  isPreviewableImageFileName,
  RESULT_PREVIEW_MAX_BYTES,
  resultDownloadUrl,
} from "@/lib/result-preview";
import { cn } from "cn";
import { useEffect, useState } from "react";

export function resultItemTitle(kind: string, name: string): string {
  return kind === "summary" ? "Assignment summary" : name;
}

type OpenResult = {
  id: string;
  title: string;
  fileName: string;
  size: number;
  kind?: string;
};

type ContentState =
  | { status: "loading" }
  | { status: "ready"; text: string }
  | { status: "image"; url: string }
  | { status: "binary" }
  | { status: "too_large" }
  | { status: "error"; message: string };

export function ResultContentDialog({
  open,
  onOpenChange,
  result,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  result: OpenResult | null;
}) {
  const [content, setContent] = useState<ContentState>({ status: "loading" });

  useEffect(() => {
    if (!open || !result) {
      return;
    }
    const controller = new AbortController();
    setContent({ status: "loading" });

    const canPreviewImage =
      isPreviewableImageFileName(result.fileName) && result.size <= RESULT_PREVIEW_MAX_BYTES;
    if (canPreviewImage) {
      setContent({ status: "image", url: resultDownloadUrl(result.id, true) });
      return () => controller.abort();
    }

    fetch(`/api/results/${result.id}/content`, { signal: controller.signal })
      .then(async (response) => {
        const body = await response.json();
        if (!response.ok) {
          if (response.status === 413) {
            setContent({ status: "too_large" });
            return;
          }
          setContent({
            status: "error",
            message:
              body.error === "unauthorized"
                ? "Sign in to view this file."
                : "This file could not be opened right now.",
          });
          return;
        }
        if (body.isBinary) {
          if (isPreviewableImageFileName(result.fileName) && result.size <= RESULT_PREVIEW_MAX_BYTES) {
            setContent({ status: "image", url: resultDownloadUrl(result.id, true) });
            return;
          }
          setContent({ status: "binary" });
          return;
        }
        setContent({ status: "ready", text: String(body.text ?? "") });
      })
      .catch((err) => {
        if (controller.signal.aborted) {
          return;
        }
        setContent({
          status: "error",
          message: err instanceof Error ? err.message : "Could not load file content.",
        });
      });
    return () => controller.abort();
  }, [open, result?.fileName, result?.id, result?.size]);

  const renderAsMarkdown =
    result !== null && isMarkdownResultFile(result.fileName, result.kind);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="flex max-h-[min(85vh,40rem)] w-full max-w-2xl flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl"
        showCloseButton
      >
        <DialogHeader className="shrink-0 border-b border-border/70 px-5 py-4 pr-12">
          <div className="flex items-start gap-3">
            <span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <FileText className="size-5" aria-hidden />
            </span>
            <div className="min-w-0 pt-0.5">
              <DialogTitle className="truncate text-base">{result?.title ?? "File"}</DialogTitle>
              <DialogDescription className="mt-1">
                {result ? formatBytes(result.size) : " "}
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <div className="min-h-0 flex-1 overflow-y-auto bg-[#faf9fc] px-5 py-4">
          {content.status === "loading" ? (
            <div className="flex items-center justify-center gap-2 py-16 text-sm text-muted-foreground">
              <Spinner className="size-5" />
              Loading file…
            </div>
          ) : null}

          {content.status === "ready" ? (
            renderAsMarkdown ? (
              <MarkdownContent text={content.text} />
            ) : (
              <pre
                className="whitespace-pre-wrap break-words font-mono text-[13px] leading-relaxed text-foreground"
              >
                {content.text || "(Empty file)"}
              </pre>
            )
          ) : null}

          {content.status === "image" ? (
            <div className="flex min-h-[12rem] items-center justify-center py-4">
              <img
                src={content.url}
                alt={result?.title ?? "Image preview"}
                className="max-h-[min(60vh,32rem)] max-w-full rounded-lg object-contain shadow-sm"
                onError={() =>
                  setContent({
                    status: "error",
                    message: "This image could not be loaded. Try downloading it instead.",
                  })
                }
              />
            </div>
          ) : null}

          {content.status === "binary" ? (
            <p className="py-8 text-center text-sm text-muted-foreground">
              This file is not plain text. Open the detailed work view to access it on your
              computer.
            </p>
          ) : null}

          {content.status === "too_large" ? (
            <p className="py-8 text-center text-sm text-muted-foreground">
              This file is too large to preview here. Use the detailed work view to access it.
            </p>
          ) : null}

          {content.status === "error" ? (
            <p className="py-8 text-center text-sm text-red-700" role="alert">
              {content.message}
            </p>
          ) : null}
        </div>
      </DialogContent>
    </Dialog>
  );
}

export function ResultOpenButton({
  label = "Open",
  onClick,
  className,
}: {
  label?: string;
  onClick: () => void;
  className?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "inline-flex shrink-0 items-center gap-1.5 rounded-xl px-2.5 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-primary/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/30",
        className,
      )}
      aria-label={`${label} file`}
    >
      {label}
      <ExternalLink className="size-3.5" aria-hidden />
    </button>
  );
}
