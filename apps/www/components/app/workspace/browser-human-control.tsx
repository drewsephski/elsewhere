"use client";

import { Button } from "@/components/ui/button";
import { cn } from "cn";
import {
  useEffect,
  useRef,
  type FormEvent,
  type KeyboardEvent,
  type RefObject,
} from "react";

interface BrowserHumanControlBarProps {
  enabled: boolean;
  humanActive: boolean;
  loading: boolean;
  inputBusy?: boolean;
  error: string | null;
  inputError?: string | null;
  onTakeControl: () => void;
  onReturnControl: () => void;
  onTypeText?: (text: string) => void | Promise<void>;
  onPressKey?: (key: string) => void | Promise<void>;
  typeInputRef?: RefObject<HTMLInputElement | null>;
  className?: string;
}

export function BrowserHumanControlBar({
  enabled,
  humanActive,
  loading,
  inputBusy = false,
  error,
  inputError,
  onTakeControl,
  onReturnControl,
  onTypeText,
  onPressKey,
  typeInputRef,
  className,
}: BrowserHumanControlBarProps) {
  const internalInputRef = useRef<HTMLInputElement>(null);
  const inputRef = typeInputRef ?? internalInputRef;
  const draftRef = useRef("");

  useEffect(() => {
    if (!humanActive) {
      draftRef.current = "";
      if (inputRef.current) {
        inputRef.current.value = "";
      }
      return;
    }
    inputRef.current?.focus();
  }, [humanActive, inputRef]);

  if (!enabled) {
    return null;
  }

  function flushDraft(): string {
    const text = draftRef.current;
    draftRef.current = "";
    if (inputRef.current) {
      inputRef.current.value = "";
    }
    return text;
  }

  function handleSubmitType(event: FormEvent) {
    event.preventDefault();
    const text = flushDraft();
    void (async () => {
      if (text && onTypeText) {
        await onTypeText(text);
      }
      await onPressKey?.("Enter");
    })();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.nativeEvent.isComposing) {
      return;
    }
    if (event.key === "Tab") {
      event.preventDefault();
      const text = flushDraft();
      void (async () => {
        if (text && onTypeText) {
          await onTypeText(text);
        }
        await onPressKey?.("Tab");
      })();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      flushDraft();
      void onPressKey?.("Escape");
      return;
    }
    if (event.key === "Backspace" && !draftRef.current) {
      event.preventDefault();
      void onPressKey?.("Backspace");
    }
  }

  function handleInput() {
    draftRef.current = inputRef.current?.value ?? "";
  }

  const busy = inputBusy || loading;
  const status = error ?? inputError;

  return (
    <div
      className={cn("pointer-events-none absolute inset-0 z-[6]", className)}
      data-browser-human-hud=""
    >
      <div className="flex items-start justify-between gap-2 p-2">
        {humanActive ? (
          <Button
            type="button"
            size="xs"
            variant="ghost"
            className="pointer-events-auto h-6 rounded-full bg-black/60 px-2.5 text-[11px] text-white hover:bg-black/75 hover:text-white"
            disabled={loading}
            onClick={onReturnControl}
            aria-label="Return to bot"
          >
            Return
          </Button>
        ) : (
          <Button
            type="button"
            size="xs"
            variant="ghost"
            className="pointer-events-auto h-6 rounded-full bg-black/45 px-2.5 text-[11px] text-white/90 hover:bg-black/65 hover:text-white"
            disabled={loading}
            onClick={onTakeControl}
          >
            Take control
          </Button>
        )}
      </div>
      {humanActive && onTypeText ? (
        <form
          onSubmit={handleSubmitType}
          className="pointer-events-none absolute inset-x-0 bottom-[2.75rem] flex justify-center px-3"
        >
          <input
            ref={inputRef}
            onInput={handleInput}
            onKeyDown={handleKeyDown}
            placeholder="Type"
            className="pointer-events-auto h-7 w-full max-w-[13.5rem] rounded-full border border-white/10 bg-black/70 px-3 text-[11px] text-white shadow-[0_4px_14px_rgba(0,0,0,0.28)] outline-none placeholder:text-white/40 focus-visible:ring-2 focus-visible:ring-white/50"
            aria-label="Type into the focused browser field"
            disabled={busy}
            autoComplete="off"
            data-1p-ignore
            data-lpignore="true"
          />
        </form>
      ) : null}
      {status ? (
        <p
          className="absolute inset-x-0 top-9 px-2 text-center text-[10px] text-red-300"
          role="alert"
        >
          {status}
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
