"use client";

import type { ComputerSummary } from "@/lib/api-types";
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

interface ComputerSelectProps {
  id?: string;
  label?: string;
  value: string;
  onValueChange: (value: string) => void;
  computers: ComputerSummary[];
  disabled?: boolean;
  loading?: boolean;
  allowEmpty?: boolean;
  emptyLabel?: string;
  unavailableId?: string | null;
  compact?: boolean;
  className?: string;
}

export function ComputerSelect({
  id,
  label = "Computer",
  value,
  onValueChange,
  computers,
  disabled,
  loading,
  allowEmpty = false,
  emptyLabel = "No computer assigned",
  unavailableId,
  compact,
  className,
}: ComputerSelectProps) {
  const placeholder = loading ? "Loading computers…" : "Choose a computer";
  const selectDisabled = disabled || loading || (!allowEmpty && computers.length === 0);
  const emptyValue = "__none__";
  const selectValue = value || (allowEmpty ? emptyValue : value);

  const items = useMemo(() => {
    const map: Record<string, string> = {};
    if (allowEmpty) {
      map[emptyValue] = emptyLabel;
    }
    if (unavailableId && !computers.some((item) => item.id === unavailableId)) {
      map[unavailableId] = "Current computer unavailable";
    }
    for (const computer of computers) {
      map[computer.id] = computer.displayName;
    }
    return map;
  }, [allowEmpty, computers, emptyLabel, emptyValue, unavailableId]);

  return (
    <FormItem className={className}>
      <Label
        htmlFor={id}
        className={compact ? "text-[11px] font-medium text-muted-foreground" : undefined}
      >
        {label}
      </Label>
      <Select
        items={items}
        value={selectValue}
        onValueChange={(next) => {
          if (next === null) {
            return;
          }
          onValueChange(next === emptyValue ? "" : next);
        }}
        disabled={selectDisabled}
      >
        <SelectTrigger
          id={id}
          size={compact ? "sm" : "default"}
          className="w-full min-w-0 bg-card"
        >
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent className="bg-card">
          {allowEmpty ? (
            <SelectItem value={emptyValue}>{emptyLabel}</SelectItem>
          ) : null}
          {unavailableId && !computers.some((item) => item.id === unavailableId) ? (
            <SelectItem value={unavailableId}>Current computer unavailable</SelectItem>
          ) : null}
          {computers.map((computer) => (
            <SelectItem key={computer.id} value={computer.id}>
              {computer.displayName}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </FormItem>
  );
}
