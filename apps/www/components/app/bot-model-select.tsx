"use client";

import { FormDescription, FormItem } from "@/components/ui/form-item";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  BOT_MODELS,
  DEFAULT_BOT_MODEL_ID,
  botModelsForSelect,
} from "@/lib/bot-models";
import { cn } from "cn";
import { useMemo } from "react";

interface BotModelSelectProps {
  id?: string;
  label?: string;
  value: string;
  onValueChange: (value: string) => void;
  disabled?: boolean;
  /** Header control: no field label or helper copy. */
  compact?: boolean;
  className?: string;
}

export function BotModelSelect({
  id = "bot-model",
  label = "Model",
  value,
  onValueChange,
  disabled,
  compact = false,
  className,
}: BotModelSelectProps) {
  const models = useMemo(() => botModelsForSelect(value), [value]);
  const items = useMemo(
    () => Object.fromEntries(models.map((model) => [model.id, model.label])),
    [models],
  );
  const selectValue = value.trim() || DEFAULT_BOT_MODEL_ID;

  return (
    <FormItem className={cn(compact && "gap-0", className)}>
      {compact ? null : <Label htmlFor={id}>{label}</Label>}
      <Select
        items={items}
        value={selectValue}
        onValueChange={(next) => {
          if (next) {
            onValueChange(next);
          }
        }}
        disabled={disabled}
      >
        <SelectTrigger
          id={id}
          size={compact ? "sm" : "default"}
          aria-label={label}
          className={cn(
            "min-w-0 bg-card",
            compact
              ? "h-7 w-auto max-w-[10.5rem] border-transparent bg-transparent px-2 shadow-none hover:bg-surface-hover"
              : "w-full",
          )}
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent className="bg-card" alignItemWithTrigger={!compact}>
          {models.map((model) => (
            <SelectItem key={model.id} value={model.id}>
              <span className="flex min-w-0 flex-col items-start gap-0.5">
                <span className="flex items-center gap-1.5">
                  {model.label}
                  {model.id === DEFAULT_BOT_MODEL_ID &&
                  BOT_MODELS.some((item) => item.id === model.id) ? (
                    <span className="text-[11px] font-normal text-muted-foreground">
                      Default
                    </span>
                  ) : null}
                </span>
                {compact ? null : (
                  <span className="text-[11px] leading-snug font-normal whitespace-normal text-muted-foreground">
                    {model.blurb}
                  </span>
                )}
              </span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {compact ? null : (
        <FormDescription>
          Applies to new work only. Queued and running work keeps its original model.
        </FormDescription>
      )}
    </FormItem>
  );
}
