"use client";

import { MarkdownContent } from "@/components/app/markdown-content";
import { FileText } from "@/components/icons/lucide";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { cloudHostFetch } from "@/lib/cloud-api";
import {
  workspaceFileName,
  type WorkspaceFileResponse,
} from "@/lib/computer-workspace";
import { formatBytes } from "@/lib/format";
import { isMarkdownResultFile } from "@/lib/result-preview";
import { useEffect, useState } from "react";

type ContentState =
  | { status: "loading" }
  | { status: "ready"; text: string; size: number }
  | { status: "binary"; size: number }
  | { status: "error"; message: string };

export function ComputerWorkspaceFileDialog({
  computerId,
  path,
  open,
  onOpenChange,
}: {
  computerId: string;
  path: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [content, setContent] = useState<ContentState>({ status: "loading" });
  const fileName = path ? workspaceFileName(path) : "";

  useEffect(() => {
    if (!open || !path) {
      return;
    }
    const controller = new AbortController();
    setContent({ status: "loading" });

    cloudHostFetch(
      `/v1/computers/${encodeURIComponent(computerId)}/workspace/file?path=${encodeURIComponent(path)}`,
      { signal: controller.signal },
    )
      .then(async (response) => {
        const body = await response.json().catch(() => ({}));
        if (!response.ok) {
          const message =
            typeof body.error === "string" ? body.error : "This file could not be opened.";
          setContent({ status: "error", message });
          return;
        }
        const data = body as WorkspaceFileResponse;
        if (data.isBinary) {
          setContent({ status: "binary", size: data.size });
          return;
        }
        setContent({
          status: "ready",
          text: data.text ?? "",
          size: data.size,
        });
      })
      .catch((err) => {
        if (controller.signal.aborted) {
          return;
        }
        setContent({
          status: "error",
          message: err instanceof Error ? err.message : "Could not load file.",
        });
      });

    return () => controller.abort();
  }, [computerId, open, path]);

  const renderAsMarkdown = path ? isMarkdownResultFile(fileName) : false;

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
              <DialogTitle className="truncate text-base">{fileName || "File"}</DialogTitle>
              <DialogDescription className="mt-1 truncate font-mono text-xs">
                {path ?? " "}
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
            <div className="mb-3 text-xs text-muted-foreground">{formatBytes(content.size)}</div>
          ) : null}

          {content.status === "ready" ? (
            renderAsMarkdown ? (
              <MarkdownContent text={content.text} />
            ) : (
              <pre className="whitespace-pre-wrap break-words font-mono text-[13px] leading-relaxed text-foreground">
                {content.text || "(Empty file)"}
              </pre>
            )
          ) : null}

          {content.status === "binary" ? (
            <p className="py-8 text-center text-sm text-muted-foreground">
              This file is not plain text ({formatBytes(content.size)}). Ask your bot to summarize
              or convert it if you need a preview here.
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
