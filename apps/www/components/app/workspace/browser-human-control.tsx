"use client";

import { Button } from "@/components/ui/button";
import { cn } from "cn";

interface BrowserHumanControlBarProps {
  enabled: boolean;
  humanActive: boolean;
  loading: boolean;
  error: string | null;
  onTakeControl: () => void;
  onReturnControl: () => void;
  className?: string;
}

export function BrowserHumanControlBar({
  enabled,
  humanActive,
  loading,
  error,
  onTakeControl,
  onReturnControl,
  className,
}: BrowserHumanControlBarProps) {
  if (!enabled) {
    return null;
  }

  return (
    <div className={cn("space-y-1 px-0.5", className)}>
      <div className="flex flex-wrap items-center gap-2">
        {humanActive ? (
          <>
            <span className="rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-700 dark:text-emerald-300">
              You have control
            </span>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-7 text-xs"
              disabled={loading}
              onClick={onReturnControl}
            >
              Return control to bot
            </Button>
          </>
        ) : (
          <Button
            type="button"
            size="sm"
            variant="secondary"
            className="h-7 text-xs"
            disabled={loading}
            onClick={onTakeControl}
          >
            Take control
          </Button>
        )}
      </div>
      {error ? (
        <p className="text-[10px] text-red-600" role="alert">
          {error}
        </p>
      ) : null}
      {humanActive ? (
        <p className="text-[10px] text-muted-foreground">
          Click the preview to interact. Browser mutations are paused for the bot until you return
          control.
        </p>
      ) : null}
    </div>
  );
}

export function BrowserHumanPreviewClickOverlay({
  humanActive,
  busy,
  onPreviewClick,
}: {
  humanActive: boolean;
  busy: boolean;
  onPreviewClick: (xRatio: number, yRatio: number) => void;
}) {
  if (!humanActive) {
    return null;
  }
  return (
    <div
      className={cn(
        "absolute inset-0 z-[2]",
        busy ? "cursor-wait" : "cursor-crosshair",
      )}
      onClick={(event) => {
        event.stopPropagation();
        const rect = event.currentTarget.getBoundingClientRect();
        if (rect.width <= 0 || rect.height <= 0) {
          return;
        }
        onPreviewClick(
          (event.clientX - rect.left) / rect.width,
          (event.clientY - rect.top) / rect.height,
        );
      }}
      aria-label="Click to interact with the browser"
      role="presentation"
    />
  );
}
