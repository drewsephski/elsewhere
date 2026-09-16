"use client";

import { Input } from "@/components/ui/input";
import { cn } from "cn";
import { useEffect, useRef, useState, type SyntheticEvent } from "react";

interface InlineRenameLabelProps {
  value: string;
  onCommit: (next: string) => void | Promise<void>;
  className?: string;
  inputClassName?: string;
  ariaLabel: string;
  disabled?: boolean;
  /** When true, opens the inline editor (e.g. from a context menu). */
  startEditing?: boolean;
  onEditingChange?: (editing: boolean) => void;
  /** Use inside another button (e.g. file tree row); rename via double-click or startEditing. */
  nested?: boolean;
}

export function InlineRenameLabel({
  value,
  onCommit,
  className,
  inputClassName,
  ariaLabel,
  disabled = false,
  startEditing = false,
  onEditingChange,
  nested = false,
}: InlineRenameLabelProps) {
  const [editing, setEditing] = useState(false);

  function setEditingState(next: boolean) {
    setEditing(next);
    onEditingChange?.(next);
  }

  useEffect(() => {
    if (startEditing && !disabled) {
      setEditingState(true);
    }
  }, [disabled, startEditing]);
  const [draft, setDraft] = useState(value);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  useEffect(() => {
    if (!editing) {
      return;
    }
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [editing]);

  async function handleCommit() {
    const trimmed = draft.trim();
    if (trimmed && trimmed !== value) {
      await onCommit(trimmed);
    } else {
      setDraft(value);
    }
    setEditingState(false);
  }

  function handleCancel() {
    setDraft(value);
    setEditingState(false);
  }

  function handleStartEdit(event: SyntheticEvent) {
    if (disabled) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    setEditingState(true);
  }

  if (editing) {
    return (
      <Input
        ref={inputRef}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => void handleCommit()}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            event.stopPropagation();
            void handleCommit();
          }
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            handleCancel();
          }
        }}
        maxLength={100}
        className={cn("h-7 min-w-0 px-1.5 text-[13px] font-semibold", inputClassName)}
        aria-label={ariaLabel}
      />
    );
  }

  const displayClassName = cn(
    "min-w-0 truncate text-left rounded-md px-0.5 -mx-0.5",
    !nested && "hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
    className,
  );

  if (nested) {
    return (
      <span
        onDoubleClick={handleStartEdit}
        onPointerDown={(event) => event.stopPropagation()}
        className={displayClassName}
        aria-label={ariaLabel}
      >
        {value}
      </span>
    );
  }

  return (
    <button
      type="button"
      onClick={handleStartEdit}
      onDoubleClick={handleStartEdit}
      onPointerDown={(event) => event.stopPropagation()}
      disabled={disabled}
      className={displayClassName}
      aria-label={ariaLabel}
    >
      {value}
    </button>
  );
}
