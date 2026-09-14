"use client";

import { Button } from "@/components/ui/button";
import { Archive } from "@/components/icons/lucide";
import { useState } from "react";

interface MessageDeleteButtonProps {
  onDelete: () => Promise<void>;
  disabled?: boolean;
  confirmMessage?: string;
  className?: string;
  label?: string;
}

export function MessageDeleteButton({
  onDelete,
  disabled,
  confirmMessage = "Delete this message from your chats? This cannot be undone.",
  className,
  label = "Delete",
}: MessageDeleteButtonProps) {
  const [pending, setPending] = useState(false);

  async function handleClick() {
    if (disabled || pending) {
      return;
    }
    if (!window.confirm(confirmMessage)) {
      return;
    }
    setPending(true);
    try {
      await onDelete();
    } finally {
      setPending(false);
    }
  }

  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      className={className}
      disabled={disabled || pending}
      onClick={() => void handleClick()}
      aria-label={label}
    >
      <Archive className="size-3.5" aria-hidden />
      {pending ? "Deleting…" : label}
    </Button>
  );
}
