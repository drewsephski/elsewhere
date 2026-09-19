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
import { BrowserAppDock } from "./browser-app-dock";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "cn";
import { Maximize2, Monitor, PictureInPicture2 } from "@/components/icons/lucide";
import {
  forwardRef,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";

export type BrowserPreviewVariant = "embedded" | "floating" | "work";

export interface BrowserPreviewHandle {
  expand: () => void;
  canExpand: boolean;
}

interface BrowserPreviewViewProps {
  variant: BrowserPreviewVariant;
  className?: string;
  /** When no provider (e.g. tests), pass frame state directly. */
  frame?: BrowserPreviewFrame | null;
  loading?: boolean;
  error?: string | null;
  enabled?: boolean;
  /** Flush chrome with parent card — no extra border or outer rounding. */
  chromeAttached?: boolean;
  viewportClassName?: string;
  /** Rendered directly under the screen card (embedded variant). */
  caption?: ReactNode;
}

/** Dark "idle desktop" scene shown until the first frame arrives. */
function BrowserIdleScene({
  enabled,
  variant,
}: {
  enabled: boolean;
  variant?: BrowserPreviewVariant;
}) {
  const idleCopy =
    variant === "work"
      ? "Your bot's computer. Live view appears when it opens a page."
      : enabled
        ? "Your bot's computer. Live view appears when it opens a page."
        : "Your bot's computer. Send a message to watch it here.";

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
          {idleCopy}
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
  variant,
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
  variant?: BrowserPreviewVariant;
  children: ReactNode;
}) {
  const showPlaceholder = !frame?.available || !frame?.imageDataUrl;

  return (
    <div className={cn("text-left", compact ? "max-w-full" : "w-full", className)}>
      <div
        className={cn(
          "overflow-hidden bg-card",
          attached ? "rounded-none border-0" : "rounded-lg border border-border",
        )}
      >
        {bare ? null : (
          <div
            className={cn(
              "flex items-center border-b border-border bg-surface-hover px-2 py-1",
              addressBar && "gap-1.5",
            )}
          >
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
            compact ? "aspect-[16/11]" : "min-h-[10rem] aspect-[16/10]",
            viewportClassName,
          )}
        >
          {loading && showPlaceholder ? (
            <div className="absolute inset-0 z-10 flex items-center justify-center bg-black/30">
              <Spinner className="size-4 text-muted-foreground" />
            </div>
          ) : null}
          {!frame?.imageDataUrl ? (
            <BrowserIdleScene enabled={enabled} variant={variant} />
          ) : null}
          {children}
        </div>
      </div>
    </div>
  );
}

function originFromNode(node: HTMLElement | null): string | undefined {
  if (!node) {
    return undefined;
  }
  const rect = node.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) {
    return undefined;
  }
  const x = ((rect.left + rect.width / 2) / window.innerWidth) * 100;
  const y = ((rect.top + rect.height / 2) / window.innerHeight) * 100;
  return `${x}% ${y}%`;
}

const COMPUTER_DIALOG_CLASS =
  "flex h-[min(92dvh,56rem)] max-h-[92dvh] w-[min(96vw,90rem)] max-w-[min(96vw,90rem)] sm:max-w-[min(96vw,90rem)] flex-col gap-0 overflow-hidden p-0 shadow-2xl duration-300 ease-out data-open:zoom-in-75 data-closed:zoom-out-90 motion-reduce:duration-0";

function ComputerPreviewDialog({
  open,
  onOpenChange,
  frame,
  host,
  origin,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  frame: BrowserPreviewFrame | null;
  host: string | null;
  origin?: string;
}) {
  const originStyle = origin ? ({ transformOrigin: origin } satisfies CSSProperties) : undefined;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={COMPUTER_DIALOG_CLASS}
        overlayClassName="duration-300 data-open:fade-in-0 data-closed:fade-out-0 motion-reduce:duration-0"
        style={originStyle}
      >
        <DialogHeader className="border-b border-border px-4 py-3 pr-12 text-left">
          <DialogTitle className="truncate text-base">
            {frame?.title || host || "Computer"}
          </DialogTitle>
          <DialogDescription className="truncate text-xs">
            {frame?.url ?? "Fullscreen view of your bot's computer."}
          </DialogDescription>
        </DialogHeader>
        <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden bg-black p-2 sm:p-4">
          {frame?.imageDataUrl ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={frame.imageDataUrl}
              alt={frame.title ? `Computer: ${frame.title}` : "Computer fullscreen"}
              className="max-h-full max-w-full object-contain"
            />
          ) : (
            <div className="flex min-h-[40vh] items-center justify-center text-sm text-muted-foreground">
              No page to show yet.
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

function PreviewOpenButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label="Open computer"
      title="Open computer"
      className="flex items-center gap-1.5 rounded-full bg-white/95 px-2.5 py-1 text-[11px] font-medium text-zinc-950 shadow-[0_4px_16px_rgba(0,0,0,0.35)] ring-1 ring-black/5 transition-colors hover:bg-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
    >
      <Maximize2 className="size-3.5" aria-hidden />
      Open computer
    </button>
  );
}

export const BrowserPreviewView = forwardRef<BrowserPreviewHandle, BrowserPreviewViewProps>(
  function BrowserPreviewView({
  variant,
  className,
  frame: frameProp,
  loading: loadingProp,
  error: errorProp,
  enabled: enabledProp,
  chromeAttached,
  caption,
}: BrowserPreviewViewProps, ref) {
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
        return enabled ? "Watch when your bot opens a page" : "Watch";
      case "preparing computer":
        return "Workspace starting…";
      case "preparing browser":
        return "Opening browser…";
      case "navigating":
        return "Loading page…";
      case "ready":
        return frame?.url ? previewHostname(frame.url) ?? "Live view" : "Live view";
      case "error":
        return "Live view unavailable";
      default:
        return "Browser";
    }
  }, [browserState, enabled, frame?.url]);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogOrigin, setDialogOrigin] = useState<string | undefined>();
  const previewShellRef = useRef<HTMLDivElement>(null);
  const typeInputRef = useRef<HTMLInputElement>(null);

  const host = useMemo(() => previewHostname(frame?.url ?? null), [frame?.url]);
  const addressLabel = host ?? statusLabel;
  const hasImage = Boolean(frame?.available && frame?.imageDataUrl);
  const pipOpen = ctx?.pipOpen ?? false;
  const isWork = variant === "work";
  const isEmbedded = variant === "embedded";
  const chromeCompact = variant === "floating" || isWork;
  const canOpenComputer = hasImage && !humanControl.humanActive;
  const appDock = ctx ? (
    <BrowserAppDock
      url={frame?.url}
      enabled={enabled}
      humanActive={humanControl.humanActive}
      controlLoading={humanControl.loading || humanClickBusy || humanInputBusy}
      size={isWork ? "compact" : "mini"}
      onTakeControl={() => humanControl.takeControl()}
      onNavigate={(url) => ctx.navigateBrowser(url)}
      onClose={() => ctx.closeBrowser()}
    />
  ) : null;

  function handleOpenDialog() {
    if (!hasImage) {
      return;
    }
    setDialogOrigin(originFromNode(previewShellRef.current));
    setDialogOpen(true);
  }

  function handleDialogChange(open: boolean) {
    setDialogOpen(open);
  }

  useImperativeHandle(
    ref,
    () => ({
      expand: handleOpenDialog,
      canExpand: hasImage,
    }),
    [hasImage],
  );

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
      typeInputRef.current?.focus();
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

  const humanControlHud = (
    <BrowserHumanControlBar
      enabled={enabled}
      humanActive={humanControl.humanActive}
      loading={humanControl.loading}
      inputBusy={humanInputBusy || humanClickBusy}
      error={humanControl.error}
      inputError={humanInputError ?? humanClickError}
      typeInputRef={typeInputRef}
      onTakeControl={() => void humanControl.takeControl()}
      onReturnControl={() => void humanControl.returnControl()}
      onTypeText={(text) => void handleHumanTypeText(text)}
      onPressKey={(key) => void handleHumanPressKey(key)}
    />
  );

  if (variant === "floating") {
    return (
      <div className={className}>
        <div ref={previewShellRef} className="relative">
          <PreviewChrome
            frame={frame}
            loading={loading}
            enabled={enabled}
            addressLabel={addressLabel}
            attached={chromeAttached}
            compact
            bare
            variant="floating"
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
          {humanControlHud}
          {appDock}
        </div>
        <ComputerPreviewDialog
          open={dialogOpen}
          onOpenChange={handleDialogChange}
          frame={frame}
          host={host}
          origin={dialogOrigin}
        />
      </div>
    );
  }

  if (isEmbedded && pipOpen) {
    return (
      <div
        className={cn(
          "rounded-lg border border-border bg-card px-3 py-2",
          className,
        )}
      >
        <p className="text-[11px] leading-snug text-muted-foreground">
          Preview is floating over the chat.{" "}
          <button
            type="button"
            className="font-medium text-foreground underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
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
          isEmbedded ? "relative" : "space-y-2",
          isWork && "w-full",
          className,
        )}
      >
        <div
          ref={previewShellRef}
          className={cn("relative", isEmbedded && "rounded-lg")}
        >
          <PreviewChrome
            frame={frame}
            loading={loading}
            enabled={enabled}
            addressLabel={addressLabel}
            bare={isEmbedded}
            compact={chromeCompact}
            variant={variant}
            viewportClassName={isWork ? "aspect-[16/10] min-h-[12rem]" : undefined}
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
          {canOpenComputer ? (
            <div
              className="absolute inset-0 z-[3] cursor-zoom-in rounded-lg"
              onClick={handleOpenDialog}
              aria-hidden
            />
          ) : null}
          <div className="pointer-events-none absolute inset-0 z-[4] flex items-start justify-end gap-1 p-2">
            {isEmbedded && ctx ? (
              <button
                type="button"
                onClick={() => ctx.openPip()}
                aria-label="Float preview over chat"
                title="Float preview over chat"
                className="pointer-events-auto flex size-7 items-center justify-center rounded-full bg-black/55 text-white/90 backdrop-blur-sm transition-colors hover:bg-black/70 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
              >
                <PictureInPicture2 className="size-3.5" aria-hidden />
              </button>
            ) : null}
            {canOpenComputer ? (
              <span className="pointer-events-auto">
                <PreviewOpenButton onClick={handleOpenDialog} />
              </span>
            ) : null}
          </div>
          {humanControlHud}
          {appDock}
        </div>

        {caption}

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

      <ComputerPreviewDialog
        open={dialogOpen}
        onOpenChange={handleDialogChange}
        frame={frame}
        host={host}
        origin={dialogOrigin}
      />
    </>
  );
});
