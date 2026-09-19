"use client";

import { Button } from "@/components/ui/button";
import {
  containedImagePointerRatio,
  normalizeWheelDelta,
} from "@/lib/browser-preview-utils";
import { cn } from "cn";
import {
  useEffect,
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
} from "react";

function pointerRatioFromEvent(
  clientX: number,
  clientY: number,
  surface: HTMLElement,
  image: HTMLImageElement | null | undefined,
): { x: number; y: number } | null {
  return containedImagePointerRatio(
    clientX,
    clientY,
    surface.getBoundingClientRect(),
    image?.naturalWidth ?? 0,
    image?.naturalHeight ?? 0,
  );
}

const TYPE_FLUSH_MS = 40;
const SCROLL_FLUSH_MS = 50;
const REMOTE_KEYS = new Set([
  "Enter",
  "Tab",
  "Escape",
  "Backspace",
  "Delete",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
]);

interface BrowserControlSwitcherProps {
  enabled: boolean;
  humanActive: boolean;
  loading: boolean;
  error?: string | null;
  onTakeControl: () => void;
  onReturnControl: () => void;
  className?: string;
}

export function BrowserControlSwitcher({
  enabled,
  humanActive,
  loading,
  error,
  onTakeControl,
  onReturnControl,
  className,
}: BrowserControlSwitcherProps) {
  if (!enabled) {
    return null;
  }

  return (
    <div className={cn("flex min-w-0 items-center gap-1", className)}>
      <span
        className={cn(
          "inline-flex min-w-0 items-center gap-1 rounded-full px-1.5 py-0.5 text-[10px] font-medium leading-none",
          humanActive
            ? "bg-success/15 text-success"
            : "bg-muted text-muted-foreground",
        )}
      >
        <span
          className={cn(
            "size-1.5 shrink-0 rounded-full",
            humanActive ? "animate-pulse bg-success" : "bg-muted-foreground/70",
          )}
          aria-hidden
        />
        <span className="truncate">{humanActive ? "You're in control" : "Bot is driving"}</span>
      </span>
      {humanActive ? (
        <Button
          type="button"
          size="xs"
          variant="outline"
          className="h-6 shrink-0 px-1.5 text-[10px] leading-none"
          disabled={loading}
          onClick={onReturnControl}
        >
          Return to bot
        </Button>
      ) : (
        <Button
          type="button"
          size="xs"
          className="h-6 shrink-0 px-1.5 text-[10px] leading-none"
          disabled={loading}
          onClick={onTakeControl}
        >
          Take control
        </Button>
      )}
      {error ? (
        <span className="sr-only" role="alert">
          {error}
        </span>
      ) : null}
    </div>
  );
}

interface BrowserRemoteSurfaceProps {
  enabled: boolean;
  humanActive: boolean;
  busy: boolean;
  inactive?: boolean;
  imageRef?: RefObject<HTMLImageElement | null>;
  onTakeControl: () => void | Promise<void>;
  onPreviewClick: (xRatio: number, yRatio: number) => void | Promise<void>;
  onTypeText: (text: string) => void | Promise<void>;
  onPressKey: (key: string) => void | Promise<void>;
  onScroll: (
    xRatio: number,
    yRatio: number,
    deltaX: number,
    deltaY: number,
  ) => void | Promise<void>;
}

export function BrowserRemoteSurface({
  enabled,
  humanActive,
  busy,
  inactive = false,
  imageRef,
  onTakeControl,
  onPreviewClick,
  onTypeText,
  onPressKey,
  onScroll,
}: BrowserRemoteSurfaceProps) {
  const surfaceRef = useRef<HTMLDivElement>(null);
  const draftRef = useRef("");
  const typeTimerRef = useRef<number | null>(null);
  const scrollRef = useRef({ x: 0.5, y: 0.5, deltaX: 0, deltaY: 0 });
  const scrollTimerRef = useRef<number | null>(null);
  const humanActiveRef = useRef(humanActive);
  const onTypeTextRef = useRef(onTypeText);
  const onScrollRef = useRef(onScroll);
  const imageRefLatest = useRef(imageRef);
  const queueScrollRef = useRef<(x: number, y: number, deltaX: number, deltaY: number) => void>(
    () => undefined,
  );

  useEffect(() => {
    humanActiveRef.current = humanActive;
  }, [humanActive]);

  useEffect(() => {
    onTypeTextRef.current = onTypeText;
  }, [onTypeText]);

  useEffect(() => {
    onScrollRef.current = onScroll;
  }, [onScroll]);

  useEffect(() => {
    imageRefLatest.current = imageRef;
  }, [imageRef]);

  queueScrollRef.current = (x, y, deltaX, deltaY) => {
    scrollRef.current.x = x;
    scrollRef.current.y = y;
    scrollRef.current.deltaX += deltaX;
    scrollRef.current.deltaY += deltaY;
    if (scrollTimerRef.current !== null) {
      return;
    }
    scrollTimerRef.current = window.setTimeout(() => {
      scrollTimerRef.current = null;
      const pending = scrollRef.current;
      scrollRef.current = { x: pending.x, y: pending.y, deltaX: 0, deltaY: 0 };
      if (pending.deltaX === 0 && pending.deltaY === 0) {
        return;
      }
      void onScrollRef.current(pending.x, pending.y, pending.deltaX, pending.deltaY);
    }, SCROLL_FLUSH_MS);
  };

  useEffect(() => {
    const surface = surfaceRef.current;
    if (!surface || !enabled || inactive) {
      return;
    }
    function handleNativeWheel(event: WheelEvent) {
      if (!humanActiveRef.current) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const viewport = surfaceRef.current;
      if (!viewport) {
        return;
      }
      const ratio =
        pointerRatioFromEvent(
          event.clientX,
          event.clientY,
          viewport,
          imageRefLatest.current?.current,
        ) ?? { x: 0.5, y: 0.5 };
      const delta = normalizeWheelDelta(event.deltaX, event.deltaY, event.deltaMode);
      if (delta.x === 0 && delta.y === 0) {
        return;
      }
      queueScrollRef.current(ratio.x, ratio.y, delta.x, delta.y);
    }
    surface.addEventListener("wheel", handleNativeWheel, { passive: false });
    return () => {
      surface.removeEventListener("wheel", handleNativeWheel);
    };
  }, [enabled, inactive, humanActive]);

  useEffect(() => {
    return () => {
      if (typeTimerRef.current !== null) {
        window.clearTimeout(typeTimerRef.current);
      }
      if (scrollTimerRef.current !== null) {
        window.clearTimeout(scrollTimerRef.current);
      }
    };
  }, []);

  if (!enabled || inactive) {
    return null;
  }

  function flushDraft(): string {
    if (typeTimerRef.current !== null) {
      window.clearTimeout(typeTimerRef.current);
      typeTimerRef.current = null;
    }
    const text = draftRef.current;
    draftRef.current = "";
    return text;
  }

  function queueType(char: string) {
    draftRef.current += char;
    if (typeTimerRef.current !== null) {
      window.clearTimeout(typeTimerRef.current);
    }
    typeTimerRef.current = window.setTimeout(() => {
      typeTimerRef.current = null;
      const text = draftRef.current;
      draftRef.current = "";
      if (text) {
        void onTypeTextRef.current(text);
      }
    }, TYPE_FLUSH_MS);
  }

  async function handlePointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    surfaceRef.current?.focus();
    if (!humanActiveRef.current) {
      await onTakeControl();
      return;
    }
    const surface = surfaceRef.current;
    if (!surface) {
      return;
    }
    const ratio = pointerRatioFromEvent(
      event.clientX,
      event.clientY,
      surface,
      imageRef?.current,
    );
    if (!ratio) {
      return;
    }
    const text = flushDraft();
    if (text) {
      await onTypeText(text);
    }
    await onPreviewClick(ratio.x, ratio.y);
  }

  function handleKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (!humanActiveRef.current || event.nativeEvent.isComposing) {
      return;
    }
    const key = event.key;
    if (event.metaKey || event.ctrlKey) {
      const shortcut = key.length === 1 ? key.toLowerCase() : key;
      if (["a", "c", "v", "x", "z", "y"].includes(shortcut)) {
        event.preventDefault();
        const modifier = event.metaKey ? "Meta" : "Control";
        const text = flushDraft();
        void (async () => {
          if (text) {
            await onTypeText(text);
          }
          await onPressKey(`${modifier}+${shortcut}`);
        })();
      }
      return;
    }
    if (event.altKey) {
      return;
    }
    if (key === " ") {
      event.preventDefault();
      const text = flushDraft();
      void (async () => {
        if (text) {
          await onTypeText(text);
        }
        await onPressKey("Space");
      })();
      return;
    }
    if (REMOTE_KEYS.has(key)) {
      event.preventDefault();
      const text = flushDraft();
      void (async () => {
        if (text) {
          await onTypeText(text);
        }
        await onPressKey(key);
      })();
      return;
    }
    if (key.length === 1) {
      event.preventDefault();
      queueType(key);
    }
  }

  const watching = !humanActive;
  return (
    <div
      ref={surfaceRef}
      tabIndex={0}
      role="application"
      aria-label={
        humanActive
          ? "Remote computer. Click, type, and scroll to control it."
          : "Remote computer preview. Click to take control."
      }
      data-browser-remote-surface=""
      className={cn(
        "absolute inset-0 z-[2] outline-none",
        watching ? "cursor-pointer" : busy ? "cursor-wait" : "cursor-default",
        humanActive &&
          "focus-visible:shadow-[inset_0_0_0_1px_color-mix(in_oklch,var(--success)_55%,transparent)]",
      )}
      onPointerDown={(event) => void handlePointerDown(event)}
      onKeyDown={handleKeyDown}
    />
  );
}
