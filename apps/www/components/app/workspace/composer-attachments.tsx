"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { MessageAttachment } from "@/lib/api-types";
import { X } from "@/components/icons/lucide";
import { useCallback, useMemo, useRef, useState } from "react";

export const COMPOSER_FILE_ACCEPT =
  "image/jpeg,image/png,image/webp,image/gif,application/pdf,text/plain,text/markdown,text/csv,application/json,.jpg,.jpeg,.png,.webp,.gif,.pdf,.txt,.md,.csv,.json";

export type StagedComposerFile = {
  localId: string;
  file: File;
  status: "uploading" | "ready" | "error";
  error?: string;
  attachment?: MessageAttachment;
  previewUrl?: string;
};

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function isImageMime(mime: string): boolean {
  return mime.startsWith("image/");
}

export function composerCanSend(text: string, files: StagedComposerFile[]): boolean {
  if (files.some((file) => file.status === "uploading")) {
    return false;
  }
  return Boolean(text.trim()) || files.some((file) => file.status === "ready");
}

export function readyAttachmentIds(files: StagedComposerFile[]): string[] {
  return files
    .filter((file) => file.status === "ready" && file.attachment)
    .map((file) => file.attachment!.id);
}

export async function uploadComposerFile(
  file: File,
  target: { botId: string } | { conversationId: string },
): Promise<MessageAttachment> {
  const body = new FormData();
  body.append("file", file, file.name);
  const path =
    "botId" in target
      ? `/v1/bots/${encodeURIComponent(target.botId)}/attachments`
      : `/v1/conversations/${encodeURIComponent(target.conversationId)}/attachments`;
  const response = await cloudHostFetch(path, {
    method: "POST",
    body,
    timeoutMs: 90_000,
  });
  if (!response.ok) {
    const payload = (await response.json().catch(() => ({}))) as { error?: string };
    throw new Error(payload.error ?? "Could not upload file");
  }
  return (await response.json()) as MessageAttachment;
}

export function ComposerAttachmentStrip({
  files,
  onRemove,
}: {
  files: StagedComposerFile[];
  onRemove: (localId: string) => void;
}) {
  if (files.length === 0) {
    return null;
  }
  return (
    <ul className="mb-2 flex flex-wrap gap-2" aria-label="Attachments">
      {files.map((item) => (
        <li
          key={item.localId}
          className="flex max-w-56 items-center gap-2 rounded-lg border border-border bg-card px-2 py-1.5 text-[12px]"
        >
          {item.previewUrl ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={item.previewUrl} alt="" className="size-8 rounded object-cover" />
          ) : (
            <span className="flex size-8 items-center justify-center rounded bg-surface-active text-[10px] uppercase text-muted-foreground">
              {item.file.name.split(".").pop()?.slice(0, 4) ?? "file"}
            </span>
          )}
          <div className="min-w-0 flex-1">
            <p className="truncate font-medium">{item.file.name}</p>
            <p className="text-muted-foreground">
              {item.status === "uploading"
                ? "Uploading…"
                : item.status === "error"
                  ? item.error
                  : formatFileSize(item.file.size)}
            </p>
          </div>
          <button
            type="button"
            className="rounded-full p-1 text-muted-foreground hover:bg-surface-hover hover:text-foreground"
            aria-label={`Remove ${item.file.name}`}
            onClick={() => onRemove(item.localId)}
          >
            <X className="size-3.5" aria-hidden />
          </button>
        </li>
      ))}
    </ul>
  );
}

export function useComposerAttachments(target: { botId: string } | { conversationId: string } | null) {
  const [files, setFiles] = useState<StagedComposerFile[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);

  const addFiles = useCallback(
    async (incoming: File[]) => {
      if (!target) {
        return;
      }
      const remaining = 4 - files.length;
      const selected = incoming.slice(0, Math.max(0, remaining));
      const staged: StagedComposerFile[] = selected.map((file) => ({
        localId: crypto.randomUUID(),
        file,
        status: "uploading",
        previewUrl: file.type.startsWith("image/") ? URL.createObjectURL(file) : undefined,
      }));
      setFiles((previous) => [...previous, ...staged]);
      for (const item of staged) {
        try {
          const attachment = await uploadComposerFile(item.file, target);
          setFiles((previous) =>
            previous.map((current) =>
              current.localId === item.localId
                ? { ...current, status: "ready", attachment }
                : current,
            ),
          );
        } catch (error) {
          setFiles((previous) =>
            previous.map((current) =>
              current.localId === item.localId
                ? {
                    ...current,
                    status: "error",
                    error: error instanceof Error ? error.message : "Upload failed",
                  }
                : current,
            ),
          );
        }
      }
    },
    [files.length, target],
  );

  const removeFile = useCallback((localId: string) => {
    setFiles((previous) => {
      const item = previous.find((file) => file.localId === localId);
      if (item?.previewUrl) {
        URL.revokeObjectURL(item.previewUrl);
      }
      return previous.filter((file) => file.localId !== localId);
    });
  }, []);

  const reset = useCallback(() => {
    setFiles((previous) => {
      for (const item of previous) {
        if (item.previewUrl) {
          URL.revokeObjectURL(item.previewUrl);
        }
      }
      return [];
    });
  }, []);

  return useMemo(
    () => ({ files, addFiles, removeFile, reset, inputRef }),
    [files, addFiles, removeFile, reset],
  );
}
