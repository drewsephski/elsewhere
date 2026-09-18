"use client";

import type { BotSummary, RunSummary } from "@/lib/api-types";
import { BotMemoryPanel } from "@/components/app/bot-memory";
import { BotRoutinesSidebar } from "./bot-routines-sidebar";
import { ComposerIconButton } from "./chat-composer";
import { ComputerStatePanel } from "./computer-state-panel";
import { ComputerWorkspaceTree } from "./computer-workspace-tree";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { cn } from "cn";
import { ChevronDown, PanelRight, Settings2 } from "@/components/icons/lucide";
import type { ReactNode } from "react";

interface BotContextRailProps {
  bot: BotSummary | null;
  activeRun: RunSummary | null;
  onBotSaved: (bot: BotSummary) => void;
  onOpenSettings?: () => void;
  onCollapseRail?: () => void;
  className?: string;
}

function RailSection({
  title,
  defaultOpen = false,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  return (
    <Collapsible
      defaultOpen={defaultOpen}
      className="group border-t border-border first:border-t-0"
    >
      <CollapsibleTrigger className="flex w-full min-w-0 cursor-pointer items-center justify-between gap-2 rounded-md py-2 text-left text-[12px] font-medium text-foreground transition-colors hover:text-foreground/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50">
        {title}
        <ChevronDown
          className="size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 group-data-open:rotate-180"
          aria-hidden
        />
      </CollapsibleTrigger>
      <CollapsibleContent className="min-w-0 pb-2">{children}</CollapsibleContent>
    </Collapsible>
  );
}

export function BotContextRail({
  bot,
  activeRun,
  onBotSaved,
  onOpenSettings,
  onCollapseRail,
  className,
}: BotContextRailProps) {
  return (
    <div className={cn("flex min-h-0 min-w-0 flex-1 flex-col", className)}>
      <div className="flex h-11 shrink-0 items-center justify-end gap-0.5 px-2">
        {onCollapseRail ? (
          <ComposerIconButton
            label="Collapse sidebar"
            className="hidden size-7 lg:flex"
            onClick={onCollapseRail}
          >
            <PanelRight className="size-4" aria-hidden />
          </ComposerIconButton>
        ) : null}
        {bot && onOpenSettings ? (
          <ComposerIconButton label="Bot settings" className="size-7" onClick={onOpenSettings}>
            <Settings2 className="size-4" aria-hidden />
          </ComposerIconButton>
        ) : null}
      </div>

      <div className="min-h-0 min-w-0 flex-1 overflow-x-hidden overflow-y-auto px-3 pb-3">
        <ComputerStatePanel bot={bot} activeRun={activeRun} variant="minimal" />

        {bot ? (
          <div className="mt-2">
            <RailSection title="Routines" defaultOpen>
              <BotRoutinesSidebar botId={bot.id} variant="minimal" />
            </RailSection>
            {bot.computerId ? (
              <RailSection title="Files">
                <ComputerWorkspaceTree computerId={bot.computerId} />
              </RailSection>
            ) : null}
            <RailSection title="Memory">
              <BotMemoryPanel
                botId={bot.id}
                learnFromConversations={Boolean(bot.learnFromConversations)}
                onLearnChanged={(enabled) => onBotSaved({ ...bot, learnFromConversations: enabled })}
                embedded
              />
            </RailSection>
          </div>
        ) : null}
      </div>
    </div>
  );
}
