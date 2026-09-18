"use client";

import { BrowserPreviewView, type BrowserPreviewHandle } from "./browser-preview-view";
import { useBrowserPreviewContext } from "@/contexts/browser-preview-context";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  clampPipPosition,
  PIP_MARGIN,
  PIP_MIN_HEIGHT,
  PIP_WIDTH,
} from "@/lib/browser-preview-pip";
import { cn } from "cn";
import {
  GripVertical,
  Maximize2,
  PanelRight,
  RefreshCw,
  X,
} from "@/components/icons/lucide";
import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";

export function FloatingBrowserPreview() {
  const ctx = useBrowserPreviewContext();
  const panelRef = useRef<HTMLDivElement>(null);
  const previewRef = useRef<BrowserPreviewHandle>(null);
  const dragOffsetRef = useRef({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const [urlDraft, setUrlDraft] = useState("");
  const [controlBusy, setControlBusy] = useState(false);
  const [controlError, setControlError] = useState<string | null>(null);

  const parentBounds = useCallback(() => {
    const parent = panelRef.current?.offsetParent;
    if (parent instanceof HTMLElement) {
      return { width: parent.clientWidth, height: parent.clientHeight };
    }
    return { width: 720, height: 480 };
  }, []);

  const panelSize = useCallback(() => {
    const rect = panelRef.current?.getBoundingClientRect();
    return {
      width: rect?.width || PIP_WIDTH,
      height: rect?.height || PIP_MIN_HEIGHT,
    };
  }, []);

  const position = ctx.pipPosition;

  useEffect(() => {
    if (ctx.frame?.url) {
      setUrlDraft(ctx.frame.url);
    }
  }, [ctx.frame?.url]);

  useEffect(() => {
    if (!ctx.pipOpen || !ctx.pipPosition) {
      return;
    }
    function handleResize() {
      if (!ctx.pipPosition) {
        return;
      }
      ctx.setPipPosition(
        clampPipPosition(ctx.pipPosition.x, ctx.pipPosition.y, parentBounds(), panelSize()),
      );
    }
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [ctx, ctx.pipOpen, ctx.pipPosition, parentBounds, panelSize]);

  const handleDock = useCallback(() => {
    ctx.dockPip();
  }, [ctx]);

  const handlePointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) {
        return;
      }
      const target = event.target as HTMLElement;
      if (target.closest("button, input, a, [data-no-drag]")) {
        return;
      }
      const panel = panelRef.current;
      const parent = panel?.offsetParent;
      if (!panel || !(parent instanceof HTMLElement)) {
        return;
      }
      const rect = panel.getBoundingClientRect();
      const parentRect = parent.getBoundingClientRect();
      dragOffsetRef.current = {
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
      };
      if (!ctx.pipPosition) {
        ctx.setPipPosition({
          x: rect.left - parentRect.left,
          y: rect.top - parentRect.top,
        });
      }
      setDragging(true);
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [ctx],
  );

  useEffect(() => {
    if (!dragging) {
      return;
    }
    function handlePointerMove(event: PointerEvent) {
      const parent = panelRef.current?.offsetParent;
      if (!(parent instanceof HTMLElement)) {
        return;
      }
      const parentRect = parent.getBoundingClientRect();
      const next = clampPipPosition(
        event.clientX - parentRect.left - dragOffsetRef.current.x,
        event.clientY - parentRect.top - dragOffsetRef.current.y,
        { width: parent.clientWidth, height: parent.clientHeight },
        panelSize(),
      );
      ctx.setPipPosition(next);
    }
    function handlePointerUp() {
      setDragging(false);
    }
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [ctx, dragging, panelSize]);

  async function handleNavigate(event: React.FormEvent) {
    event.preventDefault();
    setControlError(null);
    setControlBusy(true);
    try {
      await ctx.navigateBrowser(urlDraft);
    } catch (err) {
      setControlError(err instanceof Error ? err.message : "Navigation failed");
    } finally {
      setControlBusy(false);
    }
  }

  const addressBar = (
    <form
      onSubmit={(event) => void handleNavigate(event)}
      className="flex min-w-0 flex-1 items-center gap-1 rounded-md border border-border/50 bg-background px-1.5 py-0.5 shadow-[inset_0_1px_2px_rgba(0,0,0,0.04)]"
    >
      <span className="shrink-0 text-[9px] text-muted-foreground/80" aria-hidden>
        🔒
      </span>
      <Input
        value={urlDraft}
        onChange={(event) => setUrlDraft(event.target.value)}
        placeholder="Enter URL"
        className="h-6 min-w-0 flex-1 border-0 bg-transparent px-0 text-[10px] shadow-none focus-visible:ring-0"
        aria-label="Navigate browser to URL"
        disabled={controlBusy}
      />
      <Button
        type="submit"
        variant="ghost"
        size="sm"
        className="h-6 shrink-0 px-1.5 text-[10px] font-medium text-muted-foreground hover:text-foreground"
        disabled={controlBusy}
      >
        Go
      </Button>
    </form>
  );

  if (!ctx.pipOpen) {
    return null;
  }

  const canExpand = Boolean(previewRef.current?.canExpand || ctx.frame?.imageDataUrl);
  const style = position
    ? { left: position.x, top: position.y, right: "auto" as const }
    : { right: PIP_MARGIN, top: PIP_MARGIN };

  return (
    <div
      ref={panelRef}
      className={cn(
        "pointer-events-auto absolute z-30 w-[min(100%-1.5rem,25rem)] max-w-[25rem] select-none",
        dragging && "cursor-grabbing",
      )}
      style={style}
      role="region"
      aria-label="Floating live computer preview"
    >
      <div
        className={cn(
          "overflow-hidden rounded-xl border border-border/70 bg-background/95 shadow-lg backdrop-blur-md",
          dragging && "ring-2 ring-ring/40",
        )}
      >
        <div
          className="flex cursor-grab items-center gap-1 border-b border-border/50 px-2 py-1.5 active:cursor-grabbing"
          onPointerDown={handlePointerDown}
        >
          <GripVertical className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          <p className="min-w-0 flex-1 truncate text-[11px] font-medium text-foreground">
            Live computer
          </p>
          <div className="flex shrink-0 items-center gap-0.5" data-no-drag>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="size-7 px-0"
              onClick={() => void ctx.refresh()}
              aria-label="Refresh preview"
              title="Refresh preview"
              disabled={controlBusy}
            >
              <RefreshCw className="size-3.5" aria-hidden />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="size-7 px-0"
              onClick={handleDock}
              aria-label="Dock preview in sidebar"
              title="Dock preview in sidebar"
            >
              <PanelRight className="size-3.5" aria-hidden />
            </Button>
            {canExpand ? (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="size-7 px-0"
                onClick={() => previewRef.current?.expand()}
                aria-label="Open computer"
                title="Open computer"
              >
                <Maximize2 className="size-3.5" aria-hidden />
              </Button>
            ) : null}
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="size-7 px-0"
              onClick={() => ctx.dismissPipForSession()}
              aria-label="Hide floating preview"
              title="Hide floating preview"
            >
              <X className="size-3.5" aria-hidden />
            </Button>
          </div>
        </div>

        <div data-no-drag>
          <BrowserPreviewView
            ref={previewRef}
            variant="floating"
            chromeAttached
            addressBar={addressBar}
          />
          {controlError ? (
            <p className="border-t border-border/40 bg-muted/20 px-2 py-1 text-[10px] text-destructive" role="alert">
              {controlError}
            </p>
          ) : null}
        </div>
      </div>
    </div>
  );
}
