"use client";

import { useOptionalActiveRun } from "@/contexts/active-run-context";
import { useOptionalBrowserPreviewContext } from "@/contexts/browser-preview-context";
import type { BrowserPreviewFrame } from "@/hooks/use-browser-preview";
import { previewHostname } from "@/lib/browser-preview-utils";
import {
  clickComputerBrowserPoint,
  pressComputerBrowserKey,
  scrollComputerBrowser,
  typeComputerBrowserFocused,
} from "@/lib/browser-control";
import {
  BrowserControlSwitcher,
  BrowserRemoteSurface,
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
import { Maximize2, Monitor, PictureInPicture2 } from "@/components/icons/lucide";
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
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

const HUMAN_POLL_MS = 800;

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
      ? "Waiting for a page to open on this computer."
      : enabled
        ? "Waiting for a page to open on this computer."
        : "Send a message to watch this computer.";

  return (
    <div className="absolute inset-0 overflow-hidden bg-[#111111]" aria-hidden>
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_130%_95%_at_28%_-10%,#3b3b3b_0%,#1f1f1f_42%,#0f0f0f_100%)]" />
      <div className="absolute -left-[12%] top-[18%] h-[70%] w-[64%] -rotate-[16deg] rounded-full bg-white/[0.05] blur-2xl" />
      <div className="absolute -right-[18%] bottom-[-10%] h-[55%] w-[60%] rounded-full bg-white/[0.03] blur-3xl" />
      <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 px-6 text-center">
        <div className="flex size-9 items-center justify-center rounded-xl bg-white/[0.06] ring-1 ring-white/[0.08]">
          <Monitor className="size-4 text-muted-foreground" aria-hidden />
        </div>
        <p className="max-w-[14rem] text-[11px] leading-snug text-muted-foreground">
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
  compact,
  className,
  toolbar,
  addressBar,
  attached,
  viewportClassName,
  variant,
  humanActive,
  children,
}: {
  frame: BrowserPreviewFrame | null;
  loading: boolean;
  enabled: boolean;
  compact?: boolean;
  className?: string;
  toolbar?: ReactNode;
  addressBar?: ReactNode;
  attached?: boolean;
  viewportClassName?: string;
  variant?: BrowserPreviewVariant;
  humanActive?: boolean;
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
        {toolbar ? (
          <div className="flex min-w-0 items-center gap-1 border-b border-border bg-surface-hover px-2 py-1">
            {toolbar}
          </div>
        ) : null}
        <div
          className={cn(
            "relative flex w-full flex-col overflow-hidden bg-[#111111]",
            compact ? "aspect-[16/11]" : "min-h-[10rem] aspect-[16/10]",
            humanActive && "ring-1 ring-inset ring-success/35",
            viewportClassName,
          )}
        >
          {addressBar ? (
            <div className="relative z-[3] shrink-0 border-b border-white/10 bg-black/45 px-2 py-1">
              {addressBar}
            </div>
          ) : null}
          <div className="relative min-h-0 flex-1">
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
  enabled,
  humanActive,
  busy,
  controlLoading,
  controlError,
  onTakeControl,
  onReturnControl,
  onPreviewClick,
  onTypeText,
  onPressKey,
  onScroll,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  frame: BrowserPreviewFrame | null;
  host: string | null;
  origin?: string;
  enabled: boolean;
  humanActive: boolean;
  busy: boolean;
  controlLoading: boolean;
  controlError: string | null;
  onTakeControl: () => void;
  onReturnControl: () => void;
  onPreviewClick: (xRatio: number, yRatio: number) => void | Promise<void>;
  onTypeText: (text: string) => void | Promise<void>;
  onPressKey: (key: string) => void | Promise<void>;
  onScroll: (xRatio: number, yRatio: number, deltaX: number, deltaY: number) => void | Promise<void>;
}) {
  const originStyle = origin ? ({ transformOrigin: origin } satisfies CSSProperties) : undefined;
  const dialogImageRef = useRef<HTMLImageElement>(null);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={COMPUTER_DIALOG_CLASS}
        overlayClassName="duration-300 data-open:fade-in-0 data-closed:fade-out-0 motion-reduce:duration-0"
        style={originStyle}
      >
        <DialogHeader className="border-b border-border px-4 py-3 pr-12 text-left">
          <div className="flex min-w-0 flex-wrap items-center justify-between gap-2">
            <div className="min-w-0">
              <DialogTitle className="truncate text-base">
                {frame?.title || host || "Computer"}
              </DialogTitle>
              <DialogDescription className="truncate text-xs">
                {humanActive
                  ? "You're using this computer. Click, type, and scroll in the live view."
                  : frame?.url ?? "Watch the live view, or take control to use it yourself."}
              </DialogDescription>
            </div>
            <BrowserControlSwitcher
              enabled={enabled}
              humanActive={humanActive}
              loading={controlLoading}
              error={controlError}
              onTakeControl={onTakeControl}
              onReturnControl={onReturnControl}
            />
          </div>
        </DialogHeader>
        <div
          className={cn(
            "relative flex min-h-0 flex-1 items-center justify-center overflow-hidden bg-black p-2 sm:p-4",
            humanActive && "ring-1 ring-inset ring-success/35",
          )}
        >
          {frame?.imageDataUrl ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              ref={dialogImageRef}
              src={frame.imageDataUrl}
              alt={frame.title ? `Computer: ${frame.title}` : "Computer fullscreen"}
              className="max-h-full max-w-full object-contain"
            />
          ) : (
            <div className="flex min-h-[40vh] items-center justify-center text-sm text-muted-foreground">
              No page to show yet.
            </div>
          )}
          <BrowserRemoteSurface
            enabled={enabled}
            humanActive={humanActive}
            busy={busy}
            imageRef={dialogImageRef}
            onTakeControl={onTakeControl}
            onPreviewClick={onPreviewClick}
            onTypeText={onTypeText}
            onPressKey={onPressKey}
            onScroll={onScroll}
          />
        </div>
      </DialogContent>
    </Dialog>
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
  const [humanBusy, setHumanBusy] = useState(false);
  const [humanInputError, setHumanInputError] = useState<string | null>(null);
  const [urlDraft, setUrlDraft] = useState("");
  const [navigateBusy, setNavigateBusy] = useState(false);
  const browserToolError = activeRun?.lastBrowserToolError ?? null;
  const imageRef = useRef<HTMLImageElement>(null);

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
        return enabled ? "Waiting for a page" : "Watch";
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

  const host = useMemo(() => previewHostname(frame?.url ?? null), [frame?.url]);
  const addressLabel = host ?? statusLabel;
  const hasImage = Boolean(frame?.available && frame?.imageDataUrl);
  const pipOpen = ctx?.pipOpen ?? false;
  const isWork = variant === "work";
  const isEmbedded = variant === "embedded";
  const chromeCompact = variant === "floating" || isWork;
  const controlError = humanControl.error ?? humanInputError;

  useEffect(() => {
    if (frame?.url) {
      setUrlDraft(frame.url);
    }
  }, [frame?.url]);

  useEffect(() => {
    if (!humanControl.humanActive || !ctx?.refresh) {
      return;
    }
    const timer = window.setInterval(() => {
      void ctx.refresh();
    }, HUMAN_POLL_MS);
    return () => window.clearInterval(timer);
  }, [humanControl.humanActive, ctx]);

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

  async function handleNavigate(event: FormEvent) {
    event.preventDefault();
    if (!ctx || !humanControl.humanActive) {
      return;
    }
    setHumanInputError(null);
    setNavigateBusy(true);
    try {
      await ctx.navigateBrowser(urlDraft);
    } catch (err) {
      setHumanInputError(err instanceof Error ? err.message : "Navigation failed");
    } finally {
      setNavigateBusy(false);
    }
  }

  async function handleHumanPreviewClick(xRatio: number, yRatio: number) {
    const computerId = ctx?.computerId;
    if (!computerId || !humanControl.humanActive) {
      return;
    }
    setHumanBusy(true);
    setHumanInputError(null);
    try {
      await clickComputerBrowserPoint(computerId, xRatio, yRatio);
      await ctx?.refresh();
    } catch (err) {
      setHumanInputError(err instanceof Error ? err.message : "Click failed");
    } finally {
      setHumanBusy(false);
    }
  }

  async function handleHumanTypeText(text: string) {
    const computerId = ctx?.computerId;
    if (!computerId || !humanControl.humanActive || !text) {
      return;
    }
    setHumanBusy(true);
    setHumanInputError(null);
    try {
      await typeComputerBrowserFocused(computerId, text);
      await ctx?.refresh();
    } catch (err) {
      setHumanInputError(err instanceof Error ? err.message : "Could not type in browser");
    } finally {
      setHumanBusy(false);
    }
  }

  async function handleHumanPressKey(key: string) {
    const computerId = ctx?.computerId;
    if (!computerId || !humanControl.humanActive) {
      return;
    }
    setHumanBusy(true);
    setHumanInputError(null);
    try {
      await pressComputerBrowserKey(computerId, key);
      await ctx?.refresh();
    } catch (err) {
      setHumanInputError(err instanceof Error ? err.message : "Key press failed");
    } finally {
      setHumanBusy(false);
    }
  }

  async function handleHumanScroll(
    xRatio: number,
    yRatio: number,
    deltaX: number,
    deltaY: number,
  ) {
    const computerId = ctx?.computerId;
    if (!computerId || !humanControl.humanActive) {
      return;
    }
    setHumanBusy(true);
    setHumanInputError(null);
    try {
      await scrollComputerBrowser(computerId, xRatio, yRatio, deltaX, deltaY);
      await ctx?.refresh();
    } catch (err) {
      setHumanInputError(err instanceof Error ? err.message : "Scroll failed");
    } finally {
      setHumanBusy(false);
    }
  }

  const remoteSurface = (
    <BrowserRemoteSurface
      enabled={enabled}
      humanActive={humanControl.humanActive}
      busy={humanBusy || navigateBusy}
      inactive={dialogOpen}
      imageRef={imageRef}
      onTakeControl={() => void humanControl.takeControl()}
      onPreviewClick={(x, y) => void handleHumanPreviewClick(x, y)}
      onTypeText={(text) => void handleHumanTypeText(text)}
      onPressKey={(key) => void handleHumanPressKey(key)}
      onScroll={(x, y, dx, dy) => void handleHumanScroll(x, y, dx, dy)}
    />
  );

  const liveImage = frame?.imageDataUrl ? (
    // eslint-disable-next-line @next/next/no-img-element
    <img
      ref={imageRef}
      src={frame.imageDataUrl}
      alt={frame.title ? `Browser: ${frame.title}` : "Live browser preview"}
      className="absolute inset-0 z-[1] h-full w-full object-contain object-center"
    />
  ) : null;

  const sessionToolbar = (
    <>
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden">
        <BrowserControlSwitcher
          enabled={enabled}
          humanActive={humanControl.humanActive}
          loading={humanControl.loading}
          error={controlError}
          onTakeControl={() => void humanControl.takeControl()}
          onReturnControl={() => void humanControl.returnControl()}
        />
      </div>
      <div className="flex shrink-0 items-center gap-0.5">
        {isEmbedded && ctx ? (
          <button
            type="button"
            onClick={() => ctx.openPip()}
            aria-label="Float preview over chat"
            title="Float preview over chat"
            className="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
          >
            <PictureInPicture2 className="size-3.5" aria-hidden />
          </button>
        ) : null}
        {hasImage ? (
          <button
            type="button"
            onClick={handleOpenDialog}
            aria-label="Open computer"
            title="Open computer"
            className="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
          >
            <Maximize2 className="size-3.5" aria-hidden />
          </button>
        ) : null}
      </div>
    </>
  );

  const addressBar =
    ctx && humanControl.humanActive ? (
      <form
        onSubmit={(event) => void handleNavigate(event)}
        className="min-w-0"
      >
        <input
          value={urlDraft}
          onChange={(event) => setUrlDraft(event.target.value)}
          placeholder="Open a URL"
          className="h-6 w-full min-w-0 rounded-md border-0 bg-white/10 px-2 font-mono text-[10px] leading-none text-white/90 outline-none placeholder:text-white/40 focus-visible:ring-2 focus-visible:ring-white/30"
          aria-label="Navigate browser to URL"
          disabled={navigateBusy || humanControl.loading}
          autoComplete="off"
          data-1p-ignore
          data-lpignore="true"
        />
      </form>
    ) : (
      <p
        className="min-w-0 truncate px-1 font-mono text-[10px] leading-6 text-white/55"
        title={frame?.url ?? addressLabel}
      >
        {addressLabel}
      </p>
    );

  const dialog = (
    <ComputerPreviewDialog
      open={dialogOpen}
      onOpenChange={handleDialogChange}
      frame={frame}
      host={host}
      origin={dialogOrigin}
      enabled={enabled}
      humanActive={humanControl.humanActive}
      busy={humanBusy || navigateBusy}
      controlLoading={humanControl.loading}
      controlError={controlError}
      onTakeControl={() => void humanControl.takeControl()}
      onReturnControl={() => void humanControl.returnControl()}
      onPreviewClick={(x, y) => void handleHumanPreviewClick(x, y)}
      onTypeText={(text) => void handleHumanTypeText(text)}
      onPressKey={(key) => void handleHumanPressKey(key)}
      onScroll={(x, y, dx, dy) => void handleHumanScroll(x, y, dx, dy)}
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
            attached={chromeAttached}
            compact
            variant="floating"
            humanActive={humanControl.humanActive}
            toolbar={sessionToolbar}
            addressBar={addressBar}
          >
            {liveImage}
            {remoteSurface}
          </PreviewChrome>
          {controlError ? (
            <p className="border-t border-border/40 bg-muted/20 px-2 py-1 text-[10px] text-destructive" role="alert">
              {controlError}
            </p>
          ) : null}
        </div>
        {dialog}
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
            compact={chromeCompact}
            variant={variant}
            humanActive={humanControl.humanActive}
            viewportClassName={isWork ? "aspect-[16/10] min-h-[12rem]" : undefined}
            toolbar={sessionToolbar}
            addressBar={addressBar}
          >
            {liveImage}
            {remoteSurface}
          </PreviewChrome>
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
        {controlError && !browserToolError ? (
          <p className="mt-1.5 px-1 text-[11px] text-destructive" role="alert">
            {controlError}
          </p>
        ) : null}
      </div>

      {dialog}
    </>
  );
});
