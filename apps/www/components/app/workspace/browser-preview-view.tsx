"use client";

import { useOptionalBrowserPreviewContext } from "@/contexts/browser-preview-context";
import { useElementFullscreen } from "@/hooks/use-element-fullscreen";
import type { BrowserPreviewFrame } from "@/hooks/use-browser-preview";
import { previewHostname } from "@/lib/browser-preview-utils";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "cn";
import { ArrowUpRight, Monitor, PanelRight, X } from "@/components/icons/lucide";
import { useMemo, useState, type ReactNode } from "react";

export type BrowserPreviewVariant = "embedded" | "pip" | "work";

interface BrowserPreviewViewProps {
  variant: BrowserPreviewVariant;
  className?: string;
  /** When no provider (e.g. tests), pass frame state directly. */
  frame?: BrowserPreviewFrame | null;
  loading?: boolean;
  error?: string | null;
  enabled?: boolean;
}

function PreviewChrome({
  frame,
  loading,
  enabled,
  compact,
  children,
}: {
  frame: BrowserPreviewFrame | null;
  loading: boolean;
  enabled: boolean;
  compact?: boolean;
  children: ReactNode;
}) {
  const host = useMemo(() => previewHostname(frame?.url ?? null), [frame?.url]);
  const showPlaceholder = !frame?.available || !frame?.imageDataUrl;

  return (
    <div
      className={cn(
        "overflow-hidden rounded-xl border border-border/80 bg-[#0f0d14] text-left shadow-inner",
        compact ? "shadow-lg ring-1 ring-black/10" : "",
      )}
    >
      <div
        className={cn(
          "flex items-center gap-1.5 border-b border-white/10 px-2.5 py-1.5",
          compact && "py-1",
        )}
      >
        <span className="size-2 rounded-full bg-[#ff5f57]" aria-hidden />
        <span className="size-2 rounded-full bg-[#febc2e]" aria-hidden />
        <span className="size-2 rounded-full bg-[#28c840]" aria-hidden />
        <span className="ml-1.5 flex min-w-0 flex-1 items-center gap-1 truncate text-[10px] text-white/55">
          <Monitor className="size-3 shrink-0" aria-hidden />
          <span className="truncate">{host ?? frame?.title ?? "Waiting for a page"}</span>
        </span>
      </div>
      <div
        className={cn(
          "relative w-full bg-[#1a1625]",
          compact ? "aspect-[16/11]" : "aspect-[16/10]",
        )}
      >
        {loading && showPlaceholder ? (
          <div className="absolute inset-0 flex items-center justify-center">
            <Spinner className="size-5 text-white/50" />
          </div>
        ) : null}
        {children}
        {!frame?.imageDataUrl ? (
          <div className="flex h-full min-h-[7rem] flex-col items-center justify-center gap-2 px-4 text-center text-xs text-white/50">
            <Monitor className="size-5 opacity-60" aria-hidden />
            <p>
              {enabled
                ? "Preview appears when your bot opens a web page."
                : "Start work to watch the browser here."}
            </p>
          </div>
        ) : null}
      </div>
    </div>
  );
}

export function BrowserPreviewView({
  variant,
  className,
  frame: frameProp,
  loading: loadingProp,
  error: errorProp,
  enabled: enabledProp,
}: BrowserPreviewViewProps) {
  const ctx = useOptionalBrowserPreviewContext();
  const frame = frameProp ?? ctx?.frame ?? null;
  const loading = loadingProp ?? ctx?.loading ?? false;
  const error = errorProp ?? ctx?.error ?? null;
  const enabled = enabledProp ?? ctx?.enabled ?? false;

  const [dialogOpen, setDialogOpen] = useState(false);
  const {
    ref: fullscreenRef,
    isFullscreen,
    toggle: toggleFullscreen,
    exit: exitFullscreen,
  } = useElementFullscreen<HTMLDivElement>();

  const host = useMemo(() => previewHostname(frame?.url ?? null), [frame?.url]);
  const hasImage = Boolean(frame?.available && frame?.imageDataUrl);
  const pipOpen = ctx?.pipOpen ?? false;

  function handleOpenDialog() {
    if (!hasImage) {
      return;
    }
    setDialogOpen(true);
  }

  function handleDialogChange(open: boolean) {
    setDialogOpen(open);
    if (!open) {
      void exitFullscreen();
    }
  }

  if (variant === "embedded" && pipOpen) {
    return (
      <div className={cn("mt-3 space-y-2 px-1", className)}>
        <p className="text-[11px] text-muted-foreground">
          Browser preview is floating over your chat.{" "}
          <button
            type="button"
            className="font-medium text-foreground underline-offset-2 hover:underline"
            onClick={() => ctx?.closePip()}
          >
            Show here
          </button>
        </p>
      </div>
    );
  }

  if (variant === "pip") {
    if (!hasImage) {
      return null;
    }
    return (
      <div
        className={cn(
          "pointer-events-auto z-30 w-[min(100%,17.5rem)] sm:w-72",
          className,
        )}
        role="region"
        aria-label="Floating browser preview"
      >
        <div className="mb-1 flex items-center justify-between gap-1">
          <p className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
            Live browser
          </p>
          <div className="flex items-center gap-0.5">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-[10px]"
              onClick={handleOpenDialog}
              aria-label="Expand browser preview"
            >
              <ArrowUpRight className="size-3.5" aria-hidden />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-[10px]"
              onClick={() => void toggleFullscreen()}
              aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
            >
              {isFullscreen ? "Exit" : "Full"}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="size-7 px-0"
              onClick={() => ctx?.dismissPipForSession()}
              aria-label="Close floating preview"
            >
              <X className="size-3.5" aria-hidden />
            </Button>
          </div>
        </div>
        <div ref={fullscreenRef} className={cn(isFullscreen && "flex min-h-0 flex-1 bg-black")}>
          <PreviewChrome frame={frame} loading={loading} enabled={enabled} compact>
            {frame?.imageDataUrl ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={frame.imageDataUrl}
                alt={frame.title ? `Browser: ${frame.title}` : "Live browser preview"}
                className={cn(
                  "h-full w-full object-cover object-top",
                  isFullscreen && "object-contain",
                )}
              />
            ) : null}
          </PreviewChrome>
        </div>
      </div>
    );
  }

  return (
    <>
      <div className={cn(variant === "embedded" ? "mt-3" : "space-y-2", className)}>
        <div className="mb-1.5 flex flex-wrap items-center justify-between gap-2 px-1">
            <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
              Live browser
            </p>
            <div className="flex flex-wrap items-center gap-1">
              {variant === "embedded" && hasImage && ctx ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-7 gap-1 px-2 text-xs text-muted-foreground"
                  onClick={() => ctx.openPip()}
                  aria-label="Pop browser preview out over chat"
                >
                  <PanelRight className="size-3.5" aria-hidden />
                  Pop out
                </Button>
              ) : null}
              {hasImage ? (
                <>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-7 gap-1 px-2 text-xs text-muted-foreground"
                    onClick={handleOpenDialog}
                    aria-label="Expand browser preview"
                  >
                    <ArrowUpRight className="size-3.5" aria-hidden />
                    Expand
                  </Button>
                </>
              ) : null}
            </div>
          </div>

        <button
          type="button"
          className={cn(
            "group relative w-full text-left",
            hasImage ? "cursor-zoom-in" : "cursor-default",
          )}
          onClick={handleOpenDialog}
          disabled={!hasImage}
          aria-label={hasImage ? "Open expanded browser preview" : "Browser preview placeholder"}
        >
          <PreviewChrome frame={frame} loading={loading} enabled={enabled}>
            {frame?.imageDataUrl ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={frame.imageDataUrl}
                alt={frame.title ? `Browser: ${frame.title}` : "Live browser preview"}
                className="h-full w-full object-cover object-top"
              />
            ) : null}
            {hasImage ? (
              <div
                className="pointer-events-none absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 to-transparent px-2 pb-2 pt-6 opacity-0 transition-opacity group-hover:opacity-100"
              >
                <p className="truncate text-[10px] text-white/85">
                  {frame?.title || host || "Live page"}
                </p>
              </div>
            ) : null}
          </PreviewChrome>
        </button>

        {error ? (
          <p className="mt-1.5 px-1 text-[11px] text-red-600" role="alert">
            {error}
          </p>
        ) : null}
      </div>

      <Dialog open={dialogOpen} onOpenChange={handleDialogChange}>
        <DialogContent className="flex max-h-[92vh] w-[min(96vw,1100px)] max-w-none flex-col gap-0 overflow-hidden p-0">
          <DialogHeader className="flex flex-row items-start justify-between gap-3 border-b border-border/70 px-4 py-3 text-left">
            <div className="min-w-0 flex-1">
              <DialogTitle className="truncate text-base">
                {frame?.title || host || "Live browser"}
              </DialogTitle>
              <DialogDescription className="truncate text-xs">
                {frame?.url ?? "Updates every few seconds while work is in progress."}
              </DialogDescription>
            </div>
            {hasImage ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={() => void toggleFullscreen()}
              >
                {isFullscreen ? "Exit fullscreen" : "Fullscreen"}
              </Button>
            ) : null}
          </DialogHeader>
          <div
            ref={fullscreenRef}
            className={cn(
              "min-h-0 flex-1 overflow-auto bg-[#0f0d14] p-2 sm:p-3",
              isFullscreen && "flex items-center justify-center p-0",
            )}
          >
            {frame?.imageDataUrl ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={frame.imageDataUrl}
                alt={frame.title ? `Browser: ${frame.title}` : "Expanded browser preview"}
                className={cn(
                  "mx-auto w-full rounded-lg object-contain",
                  isFullscreen ? "max-h-full max-w-full rounded-none" : "max-h-[calc(92vh-5.5rem)]",
                )}
              />
            ) : (
              <div className="flex min-h-[40vh] items-center justify-center text-sm text-muted-foreground">
                No page to show yet.
              </div>
            )}
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
