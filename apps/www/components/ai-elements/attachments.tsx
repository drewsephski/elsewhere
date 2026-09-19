"use client";

import { FileText, X } from "@/components/icons/lucide";
import { cn } from "cn";
import {
  createContext,
  useContext,
  type HTMLAttributes,
  type ReactNode,
} from "react";

export type FileUIPartLike = {
  id: string;
  type: "file" | "source-document";
  filename?: string;
  mediaType?: string;
  title?: string;
  url: string;
};

export type AttachmentVariant = "grid" | "inline" | "list";

const AttachmentsVariantContext = createContext<AttachmentVariant>("grid");
const AttachmentItemContext = createContext<{
  data: FileUIPartLike;
  onRemove?: () => void;
} | null>(null);

function useAttachmentItem() {
  const value = useContext(AttachmentItemContext);
  if (!value) {
    throw new Error("Attachment parts must be rendered inside Attachment");
  }
  return value;
}

export function getMediaCategory(
  data: FileUIPartLike,
): "image" | "video" | "audio" | "document" | "source" | "unknown" {
  if (data.type === "source-document") {
    return "source";
  }
  const mime = data.mediaType ?? "";
  if (mime.startsWith("image/")) {
    return "image";
  }
  if (mime.startsWith("video/")) {
    return "video";
  }
  if (mime.startsWith("audio/")) {
    return "audio";
  }
  if (mime) {
    return "document";
  }
  return "unknown";
}

export function getAttachmentLabel(data: FileUIPartLike): string {
  if (data.filename?.trim()) {
    return data.filename;
  }
  if (data.title?.trim()) {
    return data.title;
  }
  const category = getMediaCategory(data);
  if (category === "image") {
    return "Image";
  }
  if (category === "source") {
    return "Source";
  }
  return "Attachment";
}

export function toFileAttachment(input: {
  id: string;
  name: string;
  mimeType?: string;
  url: string;
}): FileUIPartLike {
  return {
    id: input.id,
    type: "file",
    filename: input.name,
    mediaType: input.mimeType,
    url: input.url,
  };
}

export function Attachments({
  variant = "grid",
  className,
  ...props
}: HTMLAttributes<HTMLDivElement> & { variant?: AttachmentVariant }) {
  return (
    <AttachmentsVariantContext.Provider value={variant}>
      <div
        className={cn(
          variant === "grid" && "flex flex-wrap gap-2",
          variant === "inline" && "flex flex-wrap gap-1.5",
          variant === "list" && "flex flex-col gap-1.5",
          className,
        )}
        {...props}
      />
    </AttachmentsVariantContext.Provider>
  );
}

export function Attachment({
  data,
  onRemove,
  className,
  children,
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  data: FileUIPartLike;
  onRemove?: () => void;
}) {
  const variant = useContext(AttachmentsVariantContext);
  return (
    <AttachmentItemContext.Provider value={{ data, onRemove }}>
      <div
        className={cn(
          "group relative overflow-hidden border border-border/80 bg-card text-[12px]",
          variant === "grid" && "w-28 rounded-xl",
          variant === "inline" && "inline-flex max-w-56 items-center gap-1.5 rounded-full px-2 py-1",
          variant === "list" && "flex max-w-64 items-center gap-2 rounded-xl px-2 py-1.5",
          className,
        )}
        {...props}
      >
        {children}
      </div>
    </AttachmentItemContext.Provider>
  );
}

export function AttachmentPreview({
  fallbackIcon,
  className,
  ...props
}: HTMLAttributes<HTMLDivElement> & { fallbackIcon?: ReactNode }) {
  const { data } = useAttachmentItem();
  const variant = useContext(AttachmentsVariantContext);
  const category = getMediaCategory(data);
  const label = getAttachmentLabel(data);
  const isImage = category === "image" && Boolean(data.url);

  if (variant === "inline") {
    return (
      <div
        className={cn(
          "relative size-5 shrink-0 overflow-hidden rounded-full bg-surface-active",
          className,
        )}
        {...props}
      >
        {isImage ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src={data.url} alt="" className="size-full object-cover" />
        ) : (
          <span className="flex size-full items-center justify-center text-muted-foreground">
            {fallbackIcon ?? <FileText className="size-3" aria-hidden />}
          </span>
        )}
      </div>
    );
  }

  if (variant === "list") {
    return (
      <div
        className={cn(
          "flex size-8 shrink-0 items-center justify-center overflow-hidden rounded-md bg-surface-active",
          className,
        )}
        {...props}
      >
        {isImage ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src={data.url} alt="" className="size-full object-cover" />
        ) : (
          fallbackIcon ?? <FileText className="size-3.5 text-muted-foreground" aria-hidden />
        )}
      </div>
    );
  }

  return (
    <div className={cn("aspect-square w-full bg-surface-active", className)} {...props}>
      {isImage ? (
        // eslint-disable-next-line @next/next/no-img-element
        <img src={data.url} alt={label} className="size-full object-cover" />
      ) : (
        <div className="flex size-full items-center justify-center text-muted-foreground">
          {fallbackIcon ?? <FileText className="size-6" aria-hidden />}
        </div>
      )}
    </div>
  );
}

export function AttachmentInfo({
  showMediaType = false,
  description,
  className,
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  showMediaType?: boolean;
  description?: string;
}) {
  const { data } = useAttachmentItem();
  const variant = useContext(AttachmentsVariantContext);
  const label = getAttachmentLabel(data);
  const secondary = description ?? (showMediaType ? data.mediaType : undefined);

  return (
    <div
      className={cn(
        "min-w-0",
        variant === "grid" && "px-2 py-1.5",
        variant === "inline" && "min-w-0 flex-1",
        variant === "list" && "min-w-0 flex-1",
        className,
      )}
      {...props}
    >
      <p className="truncate font-medium text-foreground">{label}</p>
      {secondary ? (
        <p className="truncate text-[11px] text-muted-foreground">{secondary}</p>
      ) : null}
    </div>
  );
}

export function AttachmentRemove({
  label = "Remove",
  className,
  ...props
}: Omit<HTMLAttributes<HTMLButtonElement>, "onClick"> & { label?: string }) {
  const { data, onRemove } = useAttachmentItem();
  if (!onRemove) {
    return null;
  }
  const variant = useContext(AttachmentsVariantContext);
  return (
    <button
      type="button"
      aria-label={`${label} ${getAttachmentLabel(data)}`}
      onClick={onRemove}
      className={cn(
        "rounded-full text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
        variant === "grid" &&
          "absolute top-1.5 right-1.5 bg-background/80 p-1 opacity-0 group-hover:opacity-100",
        variant === "inline" && "p-0.5",
        variant === "list" && "p-1",
        className,
      )}
      {...props}
    >
      <X className="size-3.5" aria-hidden />
    </button>
  );
}

export function AttachmentEmpty({
  className,
  ...props
}: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("text-[12px] text-muted-foreground", className)}
      {...props}
    />
  );
}
