"use client";

import type { BotSummary } from "@/lib/api-types";
import { FormItem } from "@/components/ui/form-item";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useMemo } from "react";

interface BotSelectProps {
  id?: string;
  label?: string;
  value: string;
  onValueChange: (botId: string) => void;
  bots: BotSummary[];
  disabled?: boolean;
  required?: boolean;
  placeholder?: string;
  /** Only bots with an assigned computer (routines need a computer). */
  requireComputer?: boolean;
  className?: string;
}

export function BotSelect({
  id,
  label = "Bot",
  value,
  onValueChange,
  bots,
  disabled,
  required,
  placeholder = "Choose a bot",
  requireComputer = false,
  className,
}: BotSelectProps) {
  const options = useMemo(() => {
    const list = requireComputer ? bots.filter((bot) => bot.computerId) : bots;
    return list;
  }, [bots, requireComputer]);

  const items = useMemo(
    () =>
      Object.fromEntries(options.map((bot) => [bot.id, bot.name])) as Record<
        string,
        string
      >,
    [options],
  );

  return (
    <FormItem className={className}>
      <Label htmlFor={id}>{label}</Label>
      <Select
        items={items}
        value={value || null}
        onValueChange={(next) => {
          if (next) {
            onValueChange(next);
          }
        }}
        disabled={disabled || options.length === 0}
        required={required}
      >
        <SelectTrigger id={id} className="w-full bg-card">
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent className="bg-card">
          {options.map((bot) => (
            <SelectItem key={bot.id} value={bot.id}>
              {bot.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </FormItem>
  );
}
