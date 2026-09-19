"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { MessageAttachment } from "@/lib/api-types";
import {
  Attachment,
  AttachmentInfo,
  AttachmentPreview,
  AttachmentRemove,
  Attachments,
  toFileAttachment,
} from "@/components/ai-elements/attachments";
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
    <Attachments variant="inline" aria-label="Attachments">
      {files.map((item) => (
        <Attachment
          key={item.localId}
          data={toFileAttachment({
            id: item.localId,
            name: item.file.name,
            mimeType: item.file.type,
            url: item.previewUrl ?? "",
          })}
          onRemove={() => onRemove(item.localId)}
        >
          <AttachmentPreview />
          <AttachmentInfo
            description={
              item.status === "uploading"
                ? "Uploading…"
                : item.status === "error"
                  ? item.error
                  : formatFileSize(item.file.size)
            }
          />
          <AttachmentRemove />
        </Attachment>
      ))}
    </Attachments>
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
