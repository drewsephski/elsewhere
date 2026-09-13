import { useCallback, useEffect, useRef, useState } from "react";
import type { Bot } from "@/lib/definitions";
import { BotNav } from "@/ui/bot-nav";
import { SidebarEdgeHandle } from "@/ui/sidebar-edge-handle";
import { SidebarExpandTab } from "@/ui/sidebar-expand-tab";
import { TooltipProvider } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

const SIDEBAR_COLLAPSED_KEY = "gptbot.bot-sidebar.collapsed";
const SIDEBAR_WIDTH_KEY = "gptbot.bot-sidebar.width";

const COLLAPSED_WIDTH_PX = 48;
const MIN_EXPANDED_WIDTH_PX = 216;
const MAX_WIDTH_PX = 320;
const DEFAULT_WIDTH_PX = 288;
const SNAP_COLLAPSE_PX = 88;

function readCollapsedPreference(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "true";
  } catch {
    return false;
  }
}

function readWidthPreference(): number {
  try {
    const raw = localStorage.getItem(SIDEBAR_WIDTH_KEY);
    if (!raw) {
      return DEFAULT_WIDTH_PX;
    }
    const parsed = Number.parseInt(raw, 10);
    if (!Number.isFinite(parsed)) {
      return DEFAULT_WIDTH_PX;
    }
    return Math.min(MAX_WIDTH_PX, Math.max(MIN_EXPANDED_WIDTH_PX, parsed));
  } catch {
    return DEFAULT_WIDTH_PX;
  }
}

interface BotSidebarProps {
  bots: Bot[];
  selectedBotId: string | null;
  onSelectBot: (id: string) => void;
  onCreateBot: () => void;
  onOpenSettings: () => void;
  apiKeyConfigured?: boolean;
  isStreaming?: boolean;
  onRenameBot?: (id: string, name: string) => void | Promise<void>;
  className?: string;
}

export function BotSidebar({
  bots,
  selectedBotId,
  onSelectBot,
  onCreateBot,
  onOpenSettings,
  apiKeyConfigured = false,
  isStreaming = false,
  onRenameBot,
  className,
}: BotSidebarProps) {
  const [collapsed, setCollapsed] = useState(readCollapsedPreference);
  const [widthPx, setWidthPx] = useState(readWidthPreference);
  const [resizing, setResizing] = useState(false);
  const expandedWidthRef = useRef(widthPx);

  useEffect(() => {
    try {
      localStorage.setItem(SIDEBAR_COLLAPSED_KEY, String(collapsed));
    } catch {
      /* ignore */
    }
  }, [collapsed]);

  useEffect(() => {
    if (!collapsed) {
      try {
        localStorage.setItem(SIDEBAR_WIDTH_KEY, String(widthPx));
      } catch {
        /* ignore */
      }
    }
  }, [collapsed, widthPx]);

  const handleExpand = useCallback(() => {
    setWidthPx(expandedWidthRef.current);
    setCollapsed(false);
  }, []);

  const handleToggleCollapsed = useCallback(() => {
    setCollapsed((wasCollapsed) => {
      if (wasCollapsed) {
        setWidthPx(expandedWidthRef.current);
        return false;
      }
      expandedWidthRef.current = widthPx;
      return true;
    });
  }, [widthPx]);

  const handleResizePointerDown = useCallback(
    (event: React.PointerEvent<HTMLButtonElement>) => {
      if (collapsed) {
        return;
      }

      event.preventDefault();
      const handleEl = event.currentTarget;
      const startX = event.clientX;
      const startWidth = widthPx;

      setResizing(true);
      handleEl.setPointerCapture(event.pointerId);

      function finishResize(endWidth: number, endCollapsed: boolean) {
        setResizing(false);
        setCollapsed(endCollapsed);
        if (!endCollapsed) {
          const clamped = Math.min(
            MAX_WIDTH_PX,
            Math.max(MIN_EXPANDED_WIDTH_PX, endWidth),
          );
          setWidthPx(clamped);
          expandedWidthRef.current = clamped;
        }
      }

      function handlePointerMove(moveEvent: PointerEvent) {
        const delta = moveEvent.clientX - startX;
        const nextWidth = startWidth + delta;

        if (nextWidth <= SNAP_COLLAPSE_PX) {
          expandedWidthRef.current = widthPx;
          setCollapsed(true);
          return;
        }

        const clamped = Math.min(
          MAX_WIDTH_PX,
          Math.max(MIN_EXPANDED_WIDTH_PX, nextWidth),
        );
        setWidthPx(clamped);
      }

      function handlePointerUp(upEvent: PointerEvent) {
        handleEl.releasePointerCapture(upEvent.pointerId);
        window.removeEventListener("pointermove", handlePointerMove);
        window.removeEventListener("pointerup", handlePointerUp);
        window.removeEventListener("pointercancel", handlePointerUp);

        const delta = upEvent.clientX - startX;
        const endWidth = startWidth + delta;
        if (endWidth <= SNAP_COLLAPSE_PX) {
          finishResize(COLLAPSED_WIDTH_PX, true);
          return;
        }
        finishResize(
          Math.min(
            MAX_WIDTH_PX,
            Math.max(MIN_EXPANDED_WIDTH_PX, endWidth),
          ),
          false,
        );
      }

      window.addEventListener("pointermove", handlePointerMove);
      window.addEventListener("pointerup", handlePointerUp);
      window.addEventListener("pointercancel", handlePointerUp);
    },
    [collapsed, widthPx],
  );

  const sidebarWidth = collapsed ? COLLAPSED_WIDTH_PX : widthPx;

  return (
    <aside
      className={cn(
        "relative isolate z-10 hidden h-full shrink-0 overflow-visible md:flex",
        !resizing && "transition-[width] duration-200 ease-out",
        className,
      )}
      style={{ width: sidebarWidth }}
      aria-label="Assistants sidebar"
      data-collapsed={collapsed ? "" : undefined}
    >
      <TooltipProvider delay={300}>
        <div
          className={cn(
            "flex h-full w-full min-w-0 flex-col overflow-hidden border-r border-border/50 bg-[#f4f4f5]",
            collapsed && "items-center",
          )}
        >
          <BotNav
            bots={bots}
            selectedBotId={selectedBotId}
            onSelectBot={onSelectBot}
            onCreateBot={onCreateBot}
            onOpenSettings={onOpenSettings}
            apiKeyConfigured={apiKeyConfigured}
            isStreaming={isStreaming}
            onRenameBot={onRenameBot}
            collapsed={collapsed}
            onToggleCollapsed={handleToggleCollapsed}
            className="min-h-0 w-full flex-1"
          />
        </div>
        {collapsed ? (
          <SidebarExpandTab onExpand={handleExpand} />
        ) : (
          <SidebarEdgeHandle
            resizing={resizing}
            onPointerDown={handleResizePointerDown}
          />
        )}
      </TooltipProvider>
    </aside>
  );
}
