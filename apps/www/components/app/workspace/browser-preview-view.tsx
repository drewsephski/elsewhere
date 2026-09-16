"use client";

import { useOptionalActiveRun } from "@/contexts/active-run-context";
import { useOptionalBrowserPreviewContext } from "@/contexts/browser-preview-context";
import type { BrowserPreviewFrame } from "@/hooks/use-browser-preview";
import { previewHostname } from "@/lib/browser-preview-utils";
import {
  clickComputerBrowserPoint,
  pressComputerBrowserKey,
  typeComputerBrowserFocused,
} from "@/lib/browser-control";
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
import { ArrowUpRight, Monitor, PanelRight } from "@/components/icons/lucide";
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
  /** Rendered directly under the screen card (embedded variant). */
  caption?: ReactNode;
}

/** Dark "idle desktop" scene shown until the first frame arrives. */
function BrowserIdleScene({ enabled }: { enabled: boolean }) {
  return (
    <div className="absolute inset-0 overflow-hidden bg-[#111111]" aria-hidden>
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_130%_95%_at_28%_-10%,#3b3b3b_0%,#1f1f1f_42%,#0f0f0f_100%)]" />
      <div className="absolute -left-[12%] top-[18%] h-[70%] w-[64%] -rotate-[16deg] rounded-full bg-white/[0.05] blur-2xl" />
      <div className="absolute -right-[18%] bottom-[-10%] h-[55%] w-[60%] rounded-full bg-white/[0.03] blur-3xl" />
      <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 px-6 text-center">
        <div className="flex size-9 items-center justify-center rounded-xl bg-white/[0.06] ring-1 ring-white/[0.08]">
          <Monitor className="size-4 text-muted-foreground" aria-hidden />
        </div>
        <p className="max-w-[13rem] text-[11px] leading-snug text-muted-foreground">
          {enabled
            ? "Live view appears when your bot opens a page."
            : "Send a message to watch the screen here."}
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
  bare,
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
  /** No title bar — just the rounded screen (rail card). */
  bare?: boolean;
  className?: string;
  addressBar?: ReactNode;
  attached?: boolean;
  viewportClassName?: string;
  children: ReactNode;
}) {
  const showPlaceholder = !frame?.available || !frame?.imageDataUrl;

  return (
    <div className={cn("text-left", compact ? "max-w-full" : "w-full", className)}>
      <div
        className={cn(
          "overflow-hidden bg-card",
          attached ? "rounded-none border-0" : "rounded-xl border border-border",
        )}
      >
        {bare ? null : (
          <div
            className={cn(
              "flex items-center gap-1.5 border-b border-border bg-surface-hover px-2",
              compact ? "py-1" : "py-1.5",
              addressBar && "gap-2",
            )}
          >
            <div className="flex shrink-0 items-center gap-1" aria-hidden>
              <span className="size-1.5 rounded-full bg-white/20" />
              <span className="size-1.5 rounded-full bg-white/20" />
              <span className="size-1.5 rounded-full bg-white/20" />
            </div>
            {addressBar ? (
              <div className="min-w-0 flex-1">{addressBar}</div>
            ) : (
              <span className="min-w-0 flex-1 truncate font-mono text-[10px] text-muted-foreground">
                {addressLabel}
              </span>
            )}
          </div>
        )}
        <div
          className={cn(
            "relative w-full overflow-hidden bg-[#111111]",
            compact ? "aspect-[16/11]" : "aspect-[16/10]",
            viewportClassName,
          )}
        >
          {loading && showPlaceholder ? (
            <div className="absolute inset-0 z-10 flex items-center justify-center bg-black/30">
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

function OverlayIconButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
      aria-label={label}
      title={label}
      className="flex size-6 items-center justify-center rounded-md bg-black/55 text-white/85 backdrop-blur-sm transition-colors hover:bg-black/75 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
    >
      {children}
    </button>
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
  caption,
}: BrowserPreviewViewProps) {
  const ctx = useOptionalBrowserPreviewContext();
  const activeRun = useOptionalActiveRun();
  const frame = frameProp ?? ctx?.frame ?? null;
  const loading = loadingProp ?? ctx?.loading ?? false;
  const error = errorProp ?? ctx?.error ?? null;
  const enabled = enabledProp ?? ctx?.enabled ?? false;
  const humanControl = useBrowserHumanControl(ctx?.computerId ?? null, enabled);
  const [humanClickBusy, setHumanClickBusy] = useState(false);
  const [humanClickError, setHumanClickError] = useState<string | null>(null);
  const [humanInputBusy, setHumanInputBusy] = useState(false);
  const [humanInputError, setHumanInputError] = useState<string | null>(null);
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
  const isEmbedded = variant === "embedded";
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

  async function handleHumanTypeText(text: string) {
    const computerId = ctx?.computerId;
    if (!computerId || !humanControl.humanActive || !text) {
      return;
    }
    setHumanInputBusy(true);
    setHumanInputError(null);
    try {
      await typeComputerBrowserFocused(computerId, text);
      await ctx?.refresh();
    } catch (err) {
      setHumanInputError(err instanceof Error ? err.message : "Could not type in browser");
    } finally {
      setHumanInputBusy(false);
    }
  }

  async function handleHumanPressKey(key: string) {
    const computerId = ctx?.computerId;
    if (!computerId || !humanControl.humanActive) {
      return;
    }
    setHumanInputBusy(true);
    setHumanInputError(null);
    try {
      await pressComputerBrowserKey(computerId, key);
      await ctx?.refresh();
    } catch (err) {
      setHumanInputError(err instanceof Error ? err.message : "Key press failed");
    } finally {
      setHumanInputBusy(false);
    }
  }

  if (variant === "floating") {
    return (
      <div className={className}>
        <BrowserHumanControlBar
          enabled={enabled}
          humanActive={humanControl.humanActive}
          loading={humanControl.loading}
          inputBusy={humanInputBusy || humanClickBusy}
          error={humanControl.error}
          inputError={humanInputError ?? humanClickError}
          onTakeControl={() => void humanControl.takeControl()}
          onReturnControl={() => void humanControl.returnControl()}
          onTypeText={(text) => void handleHumanTypeText(text)}
          onPressKey={(key) => void handleHumanPressKey(key)}
          className="mb-1.5 px-2 pt-1"
        />
        <PreviewChrome
          frame={frame}
          loading={loading}
          enabled={enabled}
          addressLabel={addressLabel}
          addressBar={addressBar}
          attached={chromeAttached}
          compact
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
            busy={humanClickBusy || humanInputBusy}
            onPreviewClick={(x, y) => void handleHumanPreviewClick(x, y)}
          />
        </PreviewChrome>
      </div>
    );
  }

  if (isEmbedded && pipOpen) {
    return (
      <div
        className={cn(
          "flex aspect-[16/10] flex-col items-center justify-center rounded-xl border border-dashed border-border px-5 text-center",
          className,
        )}
      >
        <p className="text-[11px] leading-snug text-muted-foreground">
          Preview is floating over the chat.{" "}
          <button
            type="button"
            className="font-medium text-foreground underline-offset-2 hover:underline"
            onClick={() => ctx?.dockPip()}
          >
            Dock it here
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
          isEmbedded ? "group/screen relative" : "space-y-2",
          isWork && "mx-auto w-full max-w-xs sm:max-w-sm",
          className,
        )}
      >
        {isWork ? (
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
        ) : null}

        <button
          type="button"
          className={cn(
            "group relative block w-full text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
            isEmbedded && "rounded-xl",
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
            bare={isEmbedded}
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
              <div className="pointer-events-none absolute inset-x-0 bottom-0 z-[2] bg-gradient-to-t from-black/75 to-transparent px-2.5 pb-2 pt-6 opacity-0 transition-opacity group-hover:opacity-100">
                <p className="truncate text-[10px] text-white/85">
                  {frame?.title || host || "Live page"}
                </p>
              </div>
            ) : null}
          </PreviewChrome>
        </button>

        {isEmbedded && hasImage ? (
          <div className="absolute top-2 right-2 z-[3] flex items-center gap-1 opacity-0 transition-opacity group-hover/screen:opacity-100 focus-within:opacity-100">
            {ctx ? (
              <OverlayIconButton label="Float preview over chat" onClick={() => ctx.openPip()}>
                <PanelRight className="size-3.5" aria-hidden />
              </OverlayIconButton>
            ) : null}
            <OverlayIconButton label="Expand preview" onClick={handleOpenDialog}>
              <ArrowUpRight className="size-3.5" aria-hidden />
            </OverlayIconButton>
          </div>
        ) : null}

        {caption}

        <BrowserHumanControlBar
          enabled={enabled}
          humanActive={humanControl.humanActive}
          loading={humanControl.loading}
          inputBusy={humanInputBusy || humanClickBusy}
          error={humanControl.error}
          inputError={humanInputError}
          onTakeControl={() => void humanControl.takeControl()}
          onReturnControl={() => void humanControl.returnControl()}
          onTypeText={(text) => void handleHumanTypeText(text)}
          onPressKey={(key) => void handleHumanPressKey(key)}
          className={isEmbedded ? "mt-1.5 flex justify-center" : "mt-1.5"}
        />

        {humanClickError ? (
          <p className="mt-1.5 px-1 text-[11px] text-destructive" role="alert">
            {humanClickError}
          </p>
        ) : null}
        {browserToolError ? (
          <div className="mt-1.5 space-y-1.5 px-1" role="alert">
            <p className="text-[11px] text-destructive">{browserToolError}</p>
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
          <p className="mt-1.5 px-1 text-[11px] text-destructive" role="alert">
            {error}
          </p>
        ) : null}
      </div>

      <Dialog open={dialogOpen} onOpenChange={handleDialogChange}>
        <DialogContent className="flex max-h-[92vh] w-[min(96vw,1100px)] max-w-none flex-col gap-0 overflow-hidden p-0">
          <DialogHeader className="border-b border-border px-4 py-3 text-left">
            <DialogTitle className="truncate text-base">
              {frame?.title || host || "Live browser"}
            </DialogTitle>
            <DialogDescription className="truncate text-xs">
              {frame?.url ?? "Updates every few seconds while work is in progress."}
            </DialogDescription>
          </DialogHeader>
          <div className="min-h-0 flex-1 overflow-auto bg-black p-2 sm:p-3">
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
