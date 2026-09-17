"use client";

import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { Button } from "@/components/ui/button";
import { Archive, Delete } from "@/components/icons/lucide";
import { messageActionCopy, type MessageActionIntent } from "@/lib/message-action-copy";
import { useState } from "react";

interface MessageDeleteButtonProps {
  onDelete: () => Promise<void>;
  disabled?: boolean;
  intent?: MessageActionIntent;
  confirmTitle?: string;
  confirmMessage?: string;
  confirmLabel?: string;
  pendingLabel?: string;
  destructive?: boolean;
  className?: string;
  label?: string;
}

export function MessageDeleteButton({
  onDelete,
  disabled,
  intent = "delete",
  confirmTitle,
  confirmMessage,
  confirmLabel,
  pendingLabel,
  destructive,
  className,
  label,
}: MessageDeleteButtonProps) {
  const defaults = messageActionCopy(intent);
  const resolvedLabel = label ?? defaults.label;
  const resolvedConfirmTitle = confirmTitle ?? defaults.confirmTitle;
  const resolvedConfirmMessage = confirmMessage ?? defaults.confirmMessage;
  const resolvedConfirmLabel = confirmLabel ?? defaults.confirmLabel;
  const resolvedPendingLabel = pendingLabel ?? defaults.pendingLabel;
  const resolvedDestructive = destructive ?? defaults.destructive;
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
        aria-label={resolvedLabel}
      >
        {intent === "archive" ? (
          <Archive className="size-3.5" aria-hidden />
        ) : (
          <Delete className="size-3.5" aria-hidden />
        )}
        {pending ? resolvedPendingLabel : resolvedLabel}
      </Button>
      <ConfirmAlertDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        title={resolvedConfirmTitle}
        description={resolvedConfirmMessage}
        confirmLabel={resolvedConfirmLabel}
        pendingLabel={resolvedPendingLabel}
        destructive={resolvedDestructive}
        pending={pending}
        onConfirm={handleConfirm}
      />
    </>
  );
}
