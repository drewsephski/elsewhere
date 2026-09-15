"use client";

import { useOptionalActiveRun } from "@/contexts/active-run-context";
import { useOptionalBrowserPreviewContext } from "@/contexts/browser-preview-context";
import type { BrowserPreviewFrame } from "@/hooks/use-browser-preview";
import { previewHostname } from "@/lib/browser-preview-utils";
import { clickComputerBrowserPoint } from "@/lib/browser-control";
import {
  BrowserHumanControlBar,
  BrowserHumanPreviewClickOverlay,
} from "./browser-human-control";
import { useBrowserHumanControl } from "@/hooks/use-browser-human-control";
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
} from "@/components/icons/lucide";
import { useMemo, useState, type ReactNode } from "react";

export type BrowserPreviewVariant = "embedded" | "floating" | "work";

interface BrowserPreviewViewProps {
  variant: BrowserPreviewVariant;
  className?: string;
  /** When no provider (e.g. tests), pass frame state directly. */
  frame?: BrowserPreviewFrame | null;
  loading?: boolean;
  error?: string | null;
  enabled?: boolean;
  /** Replaces the read-only address label in compact chrome (e.g. floating URL bar). */
  addressBar?: ReactNode;
  /** Flush chrome with parent card — no extra border or outer rounding. */
  chromeAttached?: boolean;
  viewportClassName?: string;
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
  addressLabel,
  compact,
  subtle,
  className,
  addressBar,
  attached,
  viewportClassName,
  children,
}: {
  frame: BrowserPreviewFrame | null;
  loading: boolean;
  enabled: boolean;
  addressLabel: string;
  compact?: boolean;
  subtle?: boolean;
  className?: string;
  addressBar?: ReactNode;
  attached?: boolean;
  viewportClassName?: string;
  children: ReactNode;
}) {
  const showPlaceholder = !frame?.available || !frame?.imageDataUrl;

  if (subtle) {
    return (
      <div className={cn("text-left", compact ? "max-w-full" : "w-full", className)}>
        <div
          className={cn(
            "overflow-hidden bg-background/80",
            attached
              ? "rounded-none border-0 shadow-none"
              : "rounded-lg border border-border/45 shadow-sm",
          )}
        >
          <div
            className={cn(
              "flex items-center gap-1.5 border-b border-border/40 bg-muted/30 px-2",
              compact ? "py-1" : "py-1.5",
              addressBar && "gap-2",
            )}
          >
            <div className="flex shrink-0 items-center gap-1" aria-hidden>
              <span className="size-1.5 rounded-full bg-[#ff5f57]/80" />
              <span className="size-1.5 rounded-full bg-[#febc2e]/80" />
              <span className="size-1.5 rounded-full bg-[#28c840]/80" />
            </div>
            {addressBar ? (
              <div className="min-w-0 flex-1">{addressBar}</div>
            ) : (
              <span className="min-w-0 flex-1 truncate text-[10px] text-muted-foreground">
                {addressLabel}
              </span>
            )}
          </div>
          <div
            className={cn(
              "relative w-full overflow-hidden bg-[#e8ecf4]",
              compact ? "aspect-[16/11]" : "aspect-[16/10]",
              viewportClassName,
            )}
          >
            {loading && showPlaceholder ? (
              <div className="absolute inset-0 z-10 flex items-center justify-center bg-background/30">
                <Spinner className="size-4 text-muted-foreground" />
              </div>
            ) : null}
            {!frame?.imageDataUrl ? <BrowserIdleScene enabled={enabled} /> : null}
            {children}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={cn("text-left", compact ? "max-w-full" : "w-full", className)}>
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
                viewportClassName,
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
  addressBar,
  chromeAttached,
}: BrowserPreviewViewProps) {
  const ctx = useOptionalBrowserPreviewContext();
  const activeRun = useOptionalActiveRun();
  const humanControl = useBrowserHumanControl(ctx?.computerId ?? null, enabled);
  const [humanClickBusy, setHumanClickBusy] = useState(false);
  const [humanClickError, setHumanClickError] = useState<string | null>(null);
  const frame = frameProp ?? ctx?.frame ?? null;
  const loading = loadingProp ?? ctx?.loading ?? false;
  const error = errorProp ?? ctx?.error ?? null;
  const enabled = enabledProp ?? ctx?.enabled ?? false;
  const browserToolError = activeRun?.lastBrowserToolError ?? null;

  const browserState = useMemo(() => {
    if (!enabled) {
      return "idle" as const;
    }
    if (browserToolError) {
      return "error" as const;
    }
    if (error && !frame?.imageDataUrl) {
      return "preparing computer" as const;
    }
    if (loading && !frame?.imageDataUrl) {
      return "preparing browser" as const;
    }
    if (frame?.url && !frame?.imageDataUrl) {
      return "navigating" as const;
    }
    if (frame?.imageDataUrl) {
      return "ready" as const;
    }
    return "idle" as const;
  }, [enabled, browserToolError, error, loading, frame?.imageDataUrl, frame?.url]);

  const statusLabel = useMemo(() => {
    switch (browserState) {
      case "idle":
        return enabled ? "Waiting for browser work" : "Browser idle";
      case "preparing computer":
        return "Starting computer…";
      case "preparing browser":
        return "Preparing browser…";
      case "navigating":
        return "Navigating…";
      case "ready":
        return frame?.url ? previewHostname(frame.url) ?? "Live page" : "Live page";
      case "error":
        return "Browser error";
      default:
        return "Browser";
    }
  }, [browserState, enabled, frame?.url]);

  const [dialogOpen, setDialogOpen] = useState(false);

  const host = useMemo(() => previewHostname(frame?.url ?? null), [frame?.url]);
  const addressLabel = host ?? statusLabel;
  const hasImage = Boolean(frame?.available && frame?.imageDataUrl);
  const pipOpen = ctx?.pipOpen ?? false;
  const isWork = variant === "work";
  const useSubtleChrome = variant === "embedded" || variant === "floating" || isWork;
  const chromeCompact = variant === "floating" || isWork;

  function handleOpenDialog() {
    if (!hasImage) {
      return;
    }
    setDialogOpen(true);
  }

  function handleDialogChange(open: boolean) {
    setDialogOpen(open);
  }

  async function handleHumanPreviewClick(xRatio: number, yRatio: number) {
    const computerId = ctx?.computerId;
    if (!computerId || !humanControl.humanActive) {
      return;
    }
    setHumanClickBusy(true);
    setHumanClickError(null);
    try {
      await clickComputerBrowserPoint(computerId, xRatio, yRatio);
      await ctx?.refresh();
    } catch (err) {
      setHumanClickError(err instanceof Error ? err.message : "Click failed");
    } finally {
      setHumanClickBusy(false);
    }
  }

  if (variant === "floating") {
    return (
      <PreviewChrome
        frame={frame}
        loading={loading}
        enabled={enabled}
        addressLabel={addressLabel}
        addressBar={addressBar}
        attached={chromeAttached}
        compact
        subtle
        className={className}
      >
        {frame?.imageDataUrl ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img
            src={frame.imageDataUrl}
            alt={frame.title ? `Browser: ${frame.title}` : "Live browser preview"}
            className="absolute inset-0 z-[1] h-full w-full object-cover object-top"
          />
        ) : null}
        <BrowserHumanPreviewClickOverlay
          humanActive={humanControl.humanActive}
          busy={humanClickBusy}
          onPreviewClick={(x, y) => void handleHumanPreviewClick(x, y)}
        />
      </PreviewChrome>
    );
  }

  if (variant === "embedded" && pipOpen) {
    return (
      <div className={cn("mt-3 space-y-2 px-1", className)}>
        <p className="text-[11px] text-muted-foreground">
          Preview is floating — drag it anywhere or{" "}
          <button
            type="button"
            className="font-medium text-foreground underline-offset-2 hover:underline"
            onClick={() => ctx?.dockPip()}
          >
            dock to sidebar
          </button>
          .
        </p>
      </div>
    );
  }

  return (
    <>
      <div
        className={cn(
          variant === "embedded" ? "mt-0" : "space-y-2",
          isWork && "mx-auto w-full max-w-xs sm:max-w-sm",
          className,
        )}
      >
        {!isWork ? (
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
                  aria-label="Float browser preview over chat"
                >
                  <PanelRight className="size-3.5" aria-hidden />
                  Float
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
        ) : (
          <div className="mb-1 flex items-center justify-between gap-2 px-0.5">
            <p className="text-[10px] font-medium text-muted-foreground">Browser</p>
            {hasImage ? (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-6 px-2 text-[11px] text-muted-foreground"
                onClick={handleOpenDialog}
                aria-label="Expand browser preview"
              >
                Expand
              </Button>
            ) : null}
          </div>
        )}

        <BrowserHumanControlBar
          enabled={enabled}
          humanActive={humanControl.humanActive}
          loading={humanControl.loading}
          error={humanControl.error}
          onTakeControl={() => void humanControl.takeControl()}
          onReturnControl={() => void humanControl.returnControl()}
          className="mb-1.5"
        />

        <button
          type="button"
          className={cn(
            "group relative w-full text-left",
            hasImage && !humanControl.humanActive ? "cursor-zoom-in" : "cursor-default",
          )}
          onClick={handleOpenDialog}
          disabled={!hasImage || humanControl.humanActive}
          aria-label={hasImage ? "Open expanded browser preview" : "Browser preview placeholder"}
        >
          <PreviewChrome
            frame={frame}
            loading={loading}
            enabled={enabled}
            addressLabel={addressLabel}
            subtle={useSubtleChrome}
            compact={chromeCompact}
            viewportClassName={isWork ? "aspect-[16/10] max-h-36 sm:max-h-40" : undefined}
          >
            {frame?.imageDataUrl ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={frame.imageDataUrl}
                alt={frame.title ? `Browser: ${frame.title}` : "Live browser preview"}
                className="absolute inset-0 z-[1] h-full w-full object-cover object-top"
              />
            ) : null}
            <BrowserHumanPreviewClickOverlay
              humanActive={humanControl.humanActive}
              busy={humanClickBusy}
              onPreviewClick={(x, y) => void handleHumanPreviewClick(x, y)}
            />
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

        {humanClickError ? (
          <p className="mt-1.5 px-1 text-[11px] text-red-600" role="alert">
            {humanClickError}
          </p>
        ) : null}
        {browserToolError ? (
          <div className="mt-1.5 space-y-1.5 px-1" role="alert">
            <p className="text-[11px] text-red-600">{browserToolError}</p>
            {ctx?.refresh ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 text-xs"
                onClick={() => void ctx.refresh()}
              >
                Retry preview
              </Button>
            ) : null}
          </div>
        ) : null}
        {error && !browserToolError ? (
          <p className="mt-1.5 px-1 text-[11px] text-red-600" role="alert">
            {error}
          </p>
        ) : null}
      </div>

      <Dialog open={dialogOpen} onOpenChange={handleDialogChange}>
        <DialogContent className="flex max-h-[92vh] w-[min(96vw,1100px)] max-w-none flex-col gap-0 overflow-hidden p-0">
          <DialogHeader className="border-b border-border/70 px-4 py-3 text-left">
            <DialogTitle className="truncate text-base">
              {frame?.title || host || "Live browser"}
            </DialogTitle>
            <DialogDescription className="truncate text-xs">
              {frame?.url ?? "Updates every few seconds while work is in progress."}
            </DialogDescription>
          </DialogHeader>
          <div className="min-h-0 flex-1 overflow-auto bg-[#0f0d14] p-2 sm:p-3">
            {frame?.imageDataUrl ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={frame.imageDataUrl}
                alt={frame.title ? `Browser: ${frame.title}` : "Expanded browser preview"}
                className="mx-auto max-h-[calc(92vh-5.5rem)] w-full rounded-lg object-contain"
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
