"use client";

import type { MessageAttachment } from "@/lib/api-types";
import { formatFileSize, isImageMime } from "./composer-attachments";
import {
  Attachment,
  AttachmentInfo,
  AttachmentPreview,
  Attachments,
  toFileAttachment,
} from "@/components/ai-elements/attachments";
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
  const hasImage = attachments.some((attachment) => isImageMime(attachment.mimeType));
  return (
    <Attachments
      variant={hasImage ? "grid" : "inline"}
      className={cn("mt-2", align === "end" ? "justify-end" : "justify-start")}
      aria-label="Attachments"
    >
      {attachments.map((attachment) => {
        const href = `/api/cloud/v1/attachments/${encodeURIComponent(attachment.id)}/content`;
        const data = toFileAttachment({
          id: attachment.id,
          name: attachment.originalName,
          mimeType: attachment.mimeType,
          url: href,
        });
        return (
          <a
            key={attachment.id}
            href={href}
            target="_blank"
            rel="noreferrer"
            aria-label={attachment.originalName}
            className="min-w-0"
          >
            <Attachment data={data}>
              <AttachmentPreview />
              {hasImage ? null : (
                <AttachmentInfo description={formatFileSize(attachment.sizeBytes)} />
              )}
            </Attachment>
          </a>
        );
      })}
    </Attachments>
  );
}
