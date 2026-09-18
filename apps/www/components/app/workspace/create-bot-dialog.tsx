"use client";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { CreateBotOutcome } from "@/lib/bot-quick-start";
import { useRouter } from "next/navigation";
import { CreateBotForm } from "./create-bot-form";

interface CreateBotDialogProps {
  open: boolean;
  onClose: () => void;
}

export function CreateBotDialog({ open, onClose }: CreateBotDialogProps) {
  const router = useRouter();

  function handleOutcome(outcome: CreateBotOutcome) {
    if (outcome.status === "created" || outcome.status === "started") {
      onClose();
      router.push(`/app/bots/${outcome.botId}`);
      router.refresh();
      return;
    }
    if (outcome.status === "bot_ready_run_failed") {
      onClose();
      router.push(`/app/bots/${outcome.botId}`);
      router.refresh();
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="gap-3 overflow-visible p-4 sm:max-w-lg">
        <DialogHeader className="gap-1">
          <DialogTitle className="text-base">New bot</DialogTitle>
          <DialogDescription className="text-xs">
            Name your teammate, say what they own, and optionally give them a first task.
          </DialogDescription>
        </DialogHeader>
        {open ? (
          <CreateBotForm
            showProviderCard={false}
            onOutcome={handleOutcome}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
