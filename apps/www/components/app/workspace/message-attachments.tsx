"use client";

import type { MessageAttachment } from "@/lib/api-types";
import { formatFileSize, isImageMime } from "./composer-attachments";
import { cn } from "cn";

export function MessageAttachmentList({
  attachments,
  align = "end",
}: {
  attachments: MessageAttachment[];
  align?: "end" | "start";
}) {
  if (!attachments.length) {
    return null;
  }
  return (
    <ul className={cn("mt-2 flex flex-wrap gap-2", align === "end" ? "justify-end" : "justify-start")}>
      {attachments.map((attachment) => {
        const href = `/api/cloud/v1/attachments/${encodeURIComponent(attachment.id)}/content`;
        if (isImageMime(attachment.mimeType)) {
          return (
            <li key={attachment.id}>
              <a href={href} target="_blank" rel="noreferrer" aria-label={attachment.originalName}>
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img
                  src={href}
                  alt={attachment.originalName}
                  className="max-h-40 max-w-48 rounded-lg border border-border object-cover"
                />
              </a>
            </li>
          );
        }
        return (
          <li key={attachment.id}>
            <a
              href={href}
              target="_blank"
              rel="noreferrer"
              className="flex max-w-56 items-center gap-2 rounded-lg border border-border bg-card px-2 py-1.5 text-[12px] text-foreground hover:bg-surface-hover"
            >
              <span className="truncate font-medium">{attachment.originalName}</span>
              <span className="text-muted-foreground">{formatFileSize(attachment.sizeBytes)}</span>
            </a>
          </li>
        );
      })}
    </ul>
  );
}
