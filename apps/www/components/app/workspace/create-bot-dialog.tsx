"use client";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { CreateBotOutcome } from "@/lib/bot-quick-start";
import { botConversationHref, rememberOnboardingOffer } from "@/lib/bot-onboarding";
import { settingsDialogHref } from "@/lib/settings-sections";
import { useRouter } from "next/navigation";
import { CreateBotForm } from "./create-bot-form";

interface CreateBotDialogProps {
  open: boolean;
  onClose: () => void;
}

export function CreateBotDialog({ open, onClose }: CreateBotDialogProps) {
  const router = useRouter();

  function handleOutcome(outcome: CreateBotOutcome) {
    if (outcome.status === "created") {
      rememberOnboardingOffer(outcome.botId);
      onClose();
      router.push(botConversationHref(outcome.botId, { setup: true }));
      router.refresh();
      return;
    }
    if (outcome.status === "started") {
      onClose();
      router.push(botConversationHref(outcome.botId));
      router.refresh();
      return;
    }
    if (outcome.status === "bot_ready_run_failed") {
      onClose();
      router.push(botConversationHref(outcome.botId));
      router.refresh();
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="gap-3 overflow-visible p-4 sm:max-w-lg">
        <DialogHeader className="gap-1">
          <DialogTitle className="text-base">New bot</DialogTitle>
          <DialogDescription className="text-xs">
            Pick an avatar, name your teammate, and say what they own.
          </DialogDescription>
        </DialogHeader>
        {open ? (
          <CreateBotForm
            showProviderCard={false}
            onConnectChatGpt={() => {
              onClose();
              router.push(settingsDialogHref("chatgpt"));
            }}
            onOutcome={handleOutcome}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
