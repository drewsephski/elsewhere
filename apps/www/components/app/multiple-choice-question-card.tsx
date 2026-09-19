"use client";

import { NeedsYouCard, type NeedsYouTone } from "@/components/app/needs-you-card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "cn";
import { useId, useMemo, useState, type KeyboardEvent } from "react";

export interface MultipleChoiceOption {
  id: string;
  label: string;
  description?: string;
}

export interface MultipleChoiceQuestionCardProps {
  title: string;
  prompt: string;
  helper?: string;
  options: MultipleChoiceOption[];
  selectedId?: string | null;
  allowCustom?: boolean;
  customOptionId?: string;
  customValue?: string;
  customPlaceholder?: string;
  onCustomChange?: (value: string) => void;
  onSelect: (optionId: string) => void;
  onSubmitCustom?: () => void;
  pending?: boolean;
  disabled?: boolean;
  error?: string | null;
  progress?: string;
  skipLabel?: string;
  onSkip?: () => void;
  continuation?: string;
  tone?: NeedsYouTone;
  className?: string;
}

function isCustomOption(option: MultipleChoiceOption, customOptionId?: string) {
  if (customOptionId) {
    return option.id === customOptionId;
  }
  const label = option.label.toLowerCase();
  return (
    option.id === "other" ||
    label.startsWith("other") ||
    label.startsWith("something else")
  );
}

export function MultipleChoiceQuestionCard({
  title,
  prompt,
  helper,
  options,
  selectedId = null,
  allowCustom = false,
  customOptionId,
  customValue = "",
  customPlaceholder = "Add a short answer",
  onCustomChange,
  onSelect,
  onSubmitCustom,
  pending = false,
  disabled = false,
  error = null,
  progress,
  skipLabel = "Skip for now",
  onSkip,
  continuation,
  tone = "pending",
  className,
}: MultipleChoiceQuestionCardProps) {
  const labelId = useId();
  const resolvedOptions = useMemo(() => {
    if (!allowCustom) {
      return options;
    }
    if (options.some((option) => isCustomOption(option, customOptionId))) {
      return options;
    }
    return [...options, { id: customOptionId ?? "other", label: "Something else…" }];
  }, [allowCustom, customOptionId, options]);
  const [focusIndex, setFocusIndex] = useState(() =>
    Math.max(0, resolvedOptions.findIndex((option) => option.id === selectedId)),
  );
  const locked = disabled || pending;
  const customSelected =
    allowCustom &&
    selectedId != null &&
    resolvedOptions.some((option) => option.id === selectedId && isCustomOption(option, customOptionId));

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (locked || (event.key !== "ArrowRight" && event.key !== "ArrowDown" && event.key !== "ArrowLeft" && event.key !== "ArrowUp" && event.key !== "Enter" && event.key !== " ")) {
      return;
    }
    event.preventDefault();
    if (event.key === "Enter" || event.key === " ") {
      const option = resolvedOptions[focusIndex];
      if (option) {
        onSelect(option.id);
      }
      return;
    }
    const delta = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : -1;
    const next = (focusIndex + delta + resolvedOptions.length) % resolvedOptions.length;
    setFocusIndex(next);
  }

  return (
    <NeedsYouCard
      tone={tone}
      title={title}
      reason={prompt}
      continuation={continuation}
      className={className}
      detail={
        <div className="space-y-2">
          {helper ? <p className="text-[12px] text-muted-foreground">{helper}</p> : null}
          {progress ? (
            <p className="text-[11px] text-muted-foreground">{progress}</p>
          ) : null}
        </div>
      }
      actions={
        <>
          <div
            role="radiogroup"
            aria-labelledby={labelId}
            className="flex w-full flex-col gap-2"
            onKeyDown={handleKeyDown}
          >
            <span id={labelId} className="sr-only">
              {prompt}
            </span>
            {resolvedOptions.map((option, index) => {
              const selected = selectedId === option.id;
              return (
                <button
                  key={option.id}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  disabled={locked}
                  tabIndex={locked ? -1 : index === focusIndex ? 0 : -1}
                  onFocus={() => setFocusIndex(index)}
                  onClick={() => onSelect(option.id)}
                  className={cn(
                    "rounded-lg border px-3 py-2 text-left text-[13px] transition-colors",
                    selected
                      ? "border-primary bg-primary/10 text-foreground"
                      : "border-border bg-surface-raised text-foreground hover:bg-surface-hover",
                    locked && !selected ? "opacity-50" : null,
                  )}
                >
                  <span className="font-medium">{option.label}</span>
                  {option.description ? (
                    <span className="mt-0.5 block text-[12px] text-muted-foreground">
                      {option.description}
                    </span>
                  ) : null}
                </button>
              );
            })}
          </div>
          {customSelected ? (
            <div className="flex w-full flex-col gap-2 sm:flex-row sm:items-center">
              <Input
                value={customValue}
                onChange={(event) => onCustomChange?.(event.target.value)}
                placeholder={customPlaceholder}
                disabled={locked}
                maxLength={280}
                aria-label="Custom answer"
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    onSubmitCustom?.();
                  }
                }}
              />
              <Button
                type="button"
                size="sm"
                disabled={locked || !customValue.trim()}
                onClick={() => onSubmitCustom?.()}
              >
                Continue
              </Button>
            </div>
          ) : null}
          {error ? (
            <p className="w-full text-[11px] text-destructive" role="alert">
              {error}
            </p>
          ) : null}
          {onSkip ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={pending}
              onClick={onSkip}
            >
              {skipLabel}
            </Button>
          ) : null}
        </>
      }
    />
  );
}
