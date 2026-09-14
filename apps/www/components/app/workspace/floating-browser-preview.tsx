"use client";

import { BrowserPreviewView } from "./browser-preview-view";
import { useBrowserPreviewContext } from "@/contexts/browser-preview-context";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "cn";
import {
  ArrowUpRight,
  GripVertical,
  RefreshCw,
  X,
} from "@/components/icons/lucide";
import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";

const PIP_WIDTH = 288;
const PIP_MARGIN = 12;
const FOOTER_CLEARANCE = 104;

function clampPosition(x: number, y: number) {
  const maxX = Math.max(PIP_MARGIN, window.innerWidth - PIP_WIDTH - PIP_MARGIN);
  const maxY = Math.max(PIP_MARGIN, window.innerHeight - 220 - PIP_MARGIN);
  return {
    x: Math.min(Math.max(PIP_MARGIN, x), maxX),
    y: Math.min(Math.max(PIP_MARGIN, y), maxY),
  };
}

function defaultPipPosition() {
  return clampPosition(
    window.innerWidth - PIP_WIDTH - PIP_MARGIN,
    window.innerHeight - FOOTER_CLEARANCE - 200,
  );
}

export function FloatingBrowserPreview() {
  const ctx = useBrowserPreviewContext();
  const panelRef = useRef<HTMLDivElement>(null);
  const dragOffsetRef = useRef({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const [urlDraft, setUrlDraft] = useState("");
  const [controlBusy, setControlBusy] = useState(false);
  const [controlError, setControlError] = useState<string | null>(null);

  const [anchorPosition] = useState(defaultPipPosition);
  const position = ctx.pipPosition ?? anchorPosition;

  useEffect(() => {
    if (ctx.frame?.url) {
      setUrlDraft(ctx.frame.url);
    }
  }, [ctx.frame?.url]);

  useEffect(() => {
    function handleResize() {
      if (!ctx.pipPosition) {
        return;
      }
      ctx.setPipPosition(clampPosition(ctx.pipPosition.x, ctx.pipPosition.y));
    }
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [ctx, ctx.pipPosition]);

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
      if (!panel) {
        return;
      }
      const rect = panel.getBoundingClientRect();
      dragOffsetRef.current = {
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
      };
      if (!ctx.pipPosition) {
        ctx.setPipPosition({ x: rect.left, y: rect.top });
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
      const next = clampPosition(
        event.clientX - dragOffsetRef.current.x,
        event.clientY - dragOffsetRef.current.y,
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
  }, [ctx, dragging]);

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

  async function handleResetSession() {
    setControlError(null);
    setControlBusy(true);
    try {
      await ctx.resetBrowserSession();
      setUrlDraft("");
    } catch (err) {
      setControlError(err instanceof Error ? err.message : "Could not reset session");
    } finally {
      setControlBusy(false);
    }
  }

  if (!ctx.pipOpen || !ctx.frame?.imageDataUrl) {
    return null;
  }

  return (
    <div
      ref={panelRef}
      className={cn(
        "pointer-events-auto fixed z-40 w-[min(100vw-1.5rem,18rem)] select-none sm:w-72",
        dragging && "cursor-grabbing",
      )}
      style={{ left: position.x, top: position.y }}
      role="region"
      aria-label="Floating live browser preview"
    >
      <div
        className={cn(
          "rounded-xl border border-border/60 bg-background/95 shadow-lg backdrop-blur-md",
          dragging && "ring-2 ring-primary/25",
        )}
      >
        <div
          className="flex cursor-grab items-center gap-1 border-b border-border/50 px-2 py-1.5 active:cursor-grabbing"
          onPointerDown={handlePointerDown}
        >
          <GripVertical className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          <p className="min-w-0 flex-1 truncate text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
            Live browser
          </p>
          <div className="flex shrink-0 items-center gap-0.5" data-no-drag>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="size-7 px-0"
              onClick={() => void ctx.refresh()}
              aria-label="Refresh preview"
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
            >
              <ArrowUpRight className="size-3.5" aria-hidden />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="size-7 px-0"
              onClick={() => ctx.dismissPipForSession()}
              aria-label="Hide floating preview"
            >
              <X className="size-3.5" aria-hidden />
            </Button>
          </div>
        </div>

        <div className="p-2 pt-1.5" data-no-drag>
          <form onSubmit={(event) => void handleNavigate(event)} className="mb-2 flex gap-1">
            <Input
              value={urlDraft}
              onChange={(event) => setUrlDraft(event.target.value)}
              placeholder="https://…"
              className="h-7 text-xs"
              aria-label="Navigate browser to URL"
              disabled={controlBusy}
            />
            <Button type="submit" size="sm" className="h-7 shrink-0 px-2 text-xs" disabled={controlBusy}>
              Go
            </Button>
          </form>
          <div className="flex items-center justify-between gap-2 pb-1.5">
            <button
              type="button"
              className="text-[10px] text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
              onClick={() => void handleResetSession()}
              disabled={controlBusy}
            >
              Clear browser session
            </button>
          </div>
          {controlError ? (
            <p className="mb-1.5 text-[10px] text-red-600" role="alert">{controlError}</p>
          ) : null}
          <BrowserPreviewView variant="floating" />
        </div>
      </div>
    </div>
  );
}
