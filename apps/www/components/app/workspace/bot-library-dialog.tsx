"use client";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { LibraryKind, LibraryOverlay } from "@/lib/workspace-chrome";
import { BotRoutinesSidebar } from "./bot-routines-sidebar";
import { BotSkillsSidebar } from "./bot-skills-sidebar";

const COPY: Record<LibraryKind, { title: string; description: string }> = {
  routines: {
    title: "Routines",
    description:
      "Recurring work for this Bot. Describe schedules in chat or use the links below.",
  },
  skills: {
    title: "Skills",
    description:
      "Skills attached to this Bot. Save from chat or attach versions in settings.",
  },
};

export function BotLibraryDialog({
  overlay,
  conversationId,
  onClose,
}: {
  overlay: LibraryOverlay;
  conversationId?: string;
  onClose: () => void;
}) {
  const open = overlay.status === "open";
  const copy = open ? COPY[overlay.library] : COPY.routines;
  const listConversationId =
    overlay.status === "open" ? overlay.conversationId ?? conversationId : conversationId;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <DialogContent className="w-full max-w-[calc(100%-1.5rem)] gap-3 p-4 sm:max-w-lg">
        <DialogHeader className="gap-1 pr-8">
          <DialogTitle className="text-base">{copy.title}</DialogTitle>
          <DialogDescription className="text-xs">{copy.description}</DialogDescription>
        </DialogHeader>
        {overlay.status === "open" ? (
          <div className="max-h-[min(70dvh,28rem)] overflow-y-auto overscroll-contain">
            {overlay.library === "routines" ? (
              <BotRoutinesSidebar
                botId={overlay.botId}
                conversationId={listConversationId}
                variant="minimal"
              />
            ) : (
              <BotSkillsSidebar botId={overlay.botId} variant="minimal" />
            )}
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
