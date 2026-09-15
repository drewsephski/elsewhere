"use client";

import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { Button } from "@/components/ui/button";
import { Archive } from "@/components/icons/lucide";
import { useState } from "react";

interface MessageDeleteButtonProps {
  onDelete: () => Promise<void>;
  disabled?: boolean;
  confirmTitle?: string;
  confirmMessage?: string;
  className?: string;
  label?: string;
}

export function MessageDeleteButton({
  onDelete,
  disabled,
  confirmTitle = "Delete message?",
  confirmMessage = "Delete this message from your chats? This cannot be undone.",
  className,
  label = "Delete",
}: MessageDeleteButtonProps) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [pending, setPending] = useState(false);

  function handleOpenClick() {
    if (disabled || pending) {
      return;
    }
    setDialogOpen(true);
  }

  async function handleConfirm() {
    if (disabled || pending) {
      return;
    }
    setPending(true);
    try {
      await onDelete();
      setDialogOpen(false);
    } finally {
      setPending(false);
    }
  }

  return (
    <>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={className}
        disabled={disabled || pending}
        onClick={handleOpenClick}
        aria-label={label}
      >
        <Archive className="size-3.5" aria-hidden />
        {pending ? "Deleting…" : label}
      </Button>
      <ConfirmAlertDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        title={confirmTitle}
        description={confirmMessage}
        confirmLabel="Delete"
        pendingLabel="Deleting…"
        destructive
        pending={pending}
        onConfirm={handleConfirm}
      />
    </>
  );
}
