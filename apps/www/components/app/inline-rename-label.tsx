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
}

export function InlineRenameLabel({
  value,
  onCommit,
  className,
  inputClassName,
  ariaLabel,
  disabled = false,
}: InlineRenameLabelProps) {
  const [editing, setEditing] = useState(false);
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
    setEditing(false);
  }

  function handleCancel() {
    setDraft(value);
    setEditing(false);
  }

  function handleStartEdit(event: SyntheticEvent) {
    if (disabled) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    setEditing(true);
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

  return (
    <button
      type="button"
      onClick={handleStartEdit}
      onDoubleClick={handleStartEdit}
      onPointerDown={(event) => event.stopPropagation()}
      disabled={disabled}
      className={cn(
        "min-w-0 truncate text-left rounded-md px-0.5 -mx-0.5 hover:bg-black/[0.04] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/25",
        className,
      )}
      aria-label={ariaLabel}
    >
      {value}
    </button>
  );
}
