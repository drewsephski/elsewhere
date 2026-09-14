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
import {
  ArrowUpRight,
  ChevronLeft,
  ChevronRight,
  Monitor,
  PanelRight,
  X,
} from "@/components/icons/lucide";
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

function BrowserIdleScene({ enabled }: { enabled: boolean }) {
  return (
    <div
      className="absolute inset-0 overflow-hidden bg-[#e8ecf4]"
      aria-hidden
    >
      <div
        className="absolute inset-0 bg-[radial-gradient(ellipse_120%_80%_at_50%_0%,#ffffff_0%,#e4eaf5_45%,#d4dce8_100%)]"
      />
      <div
        className="absolute -left-[20%] top-[8%] h-[55%] w-[70%] rounded-full bg-[#c8d8f0]/40 blur-3xl"
      />
      <div
        className="absolute -right-[15%] top-[25%] h-[45%] w-[55%] rounded-full bg-[#dfe8f8]/70 blur-2xl"
      />
      <div className="absolute inset-x-[12%] bottom-[18%] top-[22%] rounded-md border border-white/60 bg-white/35 shadow-[inset_0_1px_0_rgba(255,255,255,0.8)] backdrop-blur-[2px]" />
      <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 px-6 text-center">
        <div
          className="flex size-11 items-center justify-center rounded-2xl bg-white/70 shadow-sm ring-1 ring-black/[0.06]"
        >
          <Monitor className="size-5 text-[#6b7280]" aria-hidden />
        </div>
        <p className="max-w-[14rem] text-[11px] font-medium leading-snug text-[#4b5563]">
          {enabled
            ? "Live view appears when your bot opens a page."
            : "Send a message to watch the browser here."}
        </p>
      </div>
    </div>
  );
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
  const addressLabel = host ?? (enabled ? "No page yet" : "Browser idle");

  return (
    <div className={cn("text-left", compact ? "max-w-full" : "w-full")}>
      <div
        className={cn(
          "rounded-[14px] bg-gradient-to-b from-[#ececf1] via-[#e3e3e8] to-[#d8d8de] p-[5px] shadow-[0_8px_24px_-8px_rgba(15,23,42,0.22),0_2px_6px_rgba(15,23,42,0.08)] ring-1 ring-black/[0.08]",
          compact && "shadow-md",
        )}
      >
        <div className="overflow-hidden rounded-[10px] bg-[#1c1c1e] p-[3px] shadow-[inset_0_0_0_1px_rgba(255,255,255,0.06)]">
          <div className="overflow-hidden rounded-[7px] bg-[#f5f5f7]">
            <div
              className={cn(
                "flex items-center gap-1.5 border-b border-black/[0.06] bg-[linear-gradient(180deg,#fafafa_0%,#f0f0f2_100%)] px-2",
                compact ? "py-1" : "py-1.5",
              )}
            >
              <div className="flex shrink-0 items-center gap-1" aria-hidden>
                <span className="size-[9px] rounded-full bg-[#ff5f57] shadow-[inset_0_-1px_0_rgba(0,0,0,0.12)]" />
                <span className="size-[9px] rounded-full bg-[#febc2e] shadow-[inset_0_-1px_0_rgba(0,0,0,0.12)]" />
                <span className="size-[9px] rounded-full bg-[#28c840] shadow-[inset_0_-1px_0_rgba(0,0,0,0.12)]" />
              </div>
              <div className="flex min-w-0 flex-1 items-center gap-1">
                <div className="flex shrink-0 items-center gap-0.5 text-[#9ca3af]" aria-hidden>
                  <ChevronLeft className="size-3 opacity-50" />
                  <ChevronRight className="size-3 opacity-35" />
                </div>
                <div
                  className="flex min-w-0 flex-1 items-center gap-1.5 rounded-md border border-black/[0.06] bg-white px-2 py-0.5 shadow-[inset_0_1px_2px_rgba(0,0,0,0.04)]"
                >
                  <span
                    className="shrink-0 text-[9px] text-[#9ca3af]"
                    aria-hidden
                  >
                    🔒
                  </span>
                  <span className="truncate text-[10px] text-[#374151]">{addressLabel}</span>
                </div>
              </div>
            </div>
            <div
              className={cn(
                "relative w-full overflow-hidden bg-[#e8ecf4]",
                compact ? "aspect-[16/11]" : "aspect-[16/10]",
              )}
            >
              {loading && showPlaceholder ? (
                <div className="absolute inset-0 z-10 flex items-center justify-center bg-white/40 backdrop-blur-[1px]">
                  <Spinner className="size-5 text-[#6b7280]" />
                </div>
              ) : null}
              {!frame?.imageDataUrl ? <BrowserIdleScene enabled={enabled} /> : null}
              {children}
            </div>
          </div>
        </div>
        <div
          className="mx-auto mt-[3px] h-[5px] w-[42%] rounded-b-md bg-gradient-to-b from-[#c4c4c9] to-[#a8a8ae] shadow-[inset_0_1px_0_rgba(255,255,255,0.35)]"
          aria-hidden
        />
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
                  "absolute inset-0 z-[1] h-full w-full object-cover object-top",
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
      <div className={cn(variant === "embedded" ? "mt-0" : "space-y-2", className)}>
        <div className="mb-1 flex flex-wrap items-center justify-between gap-2 px-0.5">
            <p className="text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground/90">
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
                className="absolute inset-0 z-[1] h-full w-full object-cover object-top"
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
