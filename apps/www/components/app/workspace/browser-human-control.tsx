"use client";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "cn";
import { useState, type FormEvent } from "react";

interface BrowserHumanControlBarProps {
  enabled: boolean;
  humanActive: boolean;
  loading: boolean;
  inputBusy?: boolean;
  error: string | null;
  inputError?: string | null;
  onTakeControl: () => void;
  onReturnControl: () => void;
  onTypeText?: (text: string) => void;
  onPressKey?: (key: string) => void;
  className?: string;
}

const HUMAN_KEYS = [
  { label: "Enter", key: "Enter" },
  { label: "Tab", key: "Tab" },
  { label: "Esc", key: "Escape" },
  { label: "⌫", key: "Backspace" },
] as const;

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
  className,
}: BrowserHumanControlBarProps) {
  const [draft, setDraft] = useState("");

  if (!enabled) {
    return null;
  }

  function handleSubmitType(event: FormEvent) {
    event.preventDefault();
    const text = draft.trim();
    if (!text || !onTypeText) {
      return;
    }
    onTypeText(text);
    setDraft("");
  }

  return (
    <div className={cn("space-y-1 px-0.5", className)}>
      <div className="flex flex-wrap items-center gap-2">
        {humanActive ? (
          <>
            <span className="rounded-full bg-success/15 px-2 py-0.5 text-[10px] font-medium text-success">
              You have control
            </span>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-6 rounded-md px-2 text-[11px]"
              disabled={loading}
              onClick={onReturnControl}
            >
              Return to bot
            </Button>
          </>
        ) : (
          <Button
            type="button"
            size="sm"
            variant="ghost"
            className="h-6 rounded-md px-2 text-[11px] text-muted-foreground hover:text-foreground"
            disabled={loading}
            onClick={onTakeControl}
          >
            Take control
          </Button>
        )}
      </div>
      {error ? (
        <p className="text-[10px] text-destructive" role="alert">
          {error}
        </p>
      ) : null}
      {humanActive ? (
        <div className="space-y-1.5">
          <p className="text-[10px] text-muted-foreground">
            Click the preview to focus fields, then type below. Bot browser actions stay paused until
            you return control.
          </p>
          {onTypeText ? (
            <form
              onSubmit={handleSubmitType}
              className="flex items-center gap-1 rounded-md border border-border/50 bg-background/80 px-1.5 py-0.5"
            >
              <Input
                value={draft}
                onChange={(event) => setDraft(event.target.value)}
                placeholder="Type into focused field"
                className="h-7 min-w-0 flex-1 border-0 bg-transparent px-0 text-[11px] shadow-none focus-visible:ring-0"
                aria-label="Type into the focused browser field"
                disabled={inputBusy || loading}
                autoComplete="off"
                data-1p-ignore
                data-lpignore="true"
              />
              <Button
                type="submit"
                size="sm"
                variant="ghost"
                className="h-7 shrink-0 px-2 text-[10px]"
                disabled={inputBusy || loading || !draft.trim()}
              >
                Send
              </Button>
            </form>
          ) : null}
          {onPressKey ? (
            <div className="flex flex-wrap gap-1">
              {HUMAN_KEYS.map(({ label, key }) => (
                <Button
                  key={key}
                  type="button"
                  size="sm"
                  variant="outline"
                  className="h-6 px-2 text-[10px]"
                  disabled={inputBusy || loading}
                  onClick={() => onPressKey(key)}
                  aria-label={`Send ${label} to browser`}
                >
                  {label}
                </Button>
              ))}
            </div>
          ) : null}
          {inputError ? (
            <p className="text-[10px] text-destructive" role="alert">
              {inputError}
            </p>
          ) : null}
        </div>
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
