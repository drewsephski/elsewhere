import { useEffect, useRef, useState, type SyntheticEvent } from "react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

interface InlineRenameLabelProps {
  value: string;
  onCommit: (next: string) => void;
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

  function handleCommit() {
    const trimmed = draft.trim();
    if (trimmed && trimmed !== value) {
      onCommit(trimmed);
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
        onBlur={handleCommit}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            event.stopPropagation();
            handleCommit();
          }
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            handleCancel();
          }
        }}
        className={cn(
          "h-6 min-w-0 flex-1 px-1.5 py-0 text-[11px] font-medium",
          inputClassName,
        )}
        aria-label={ariaLabel}
      />
    );
  }

  return (
    <span
      role="button"
      tabIndex={disabled ? -1 : 0}
      onDoubleClick={handleStartEdit}
      onKeyDown={(event) => {
        if (disabled) {
          return;
        }
        if (event.key === "Enter" || event.key === "F2") {
          handleStartEdit(event);
        }
      }}
      className={cn(
        "min-w-0 truncate rounded px-0.5 -mx-0.5 outline-none",
        !disabled &&
          "cursor-text hover:bg-white/70 focus-visible:ring-2 focus-visible:ring-foreground/15",
        className,
      )}
      title={disabled ? undefined : "Double-click to rename"}
    >
      {value}
    </span>
  );
}
