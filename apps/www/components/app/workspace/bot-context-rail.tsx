"use client";

import type { BotSummary, RunSummary } from "@/lib/api-types";
import { BotContext } from "@/components/app/bot-context";
import { BotSettings } from "@/components/app/bot-settings";
import { ProviderStatusCard } from "@/components/app/provider-status-card";
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
import { ChevronDown, ChevronsRight, Settings2 } from "@/components/icons/lucide";
import { useRef, useState, type ReactNode } from "react";

interface BotContextRailProps {
  bot: BotSummary | null;
  activeRun: RunSummary | null;
  showConnectionSettings: boolean;
  onBotSaved: (bot: BotSummary) => void;
  /** Desktop only: hides the rail (the conversation header offers a way back). */
  onCollapse?: () => void;
  className?: string;
}

function RailSection({
  title,
  defaultOpen = false,
  open,
  onOpenChange,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  children: ReactNode;
}) {
  return (
    <Collapsible
      defaultOpen={defaultOpen}
      open={open}
      onOpenChange={onOpenChange}
      className="group border-t border-border first:border-t-0"
    >
      <CollapsibleTrigger className="flex w-full cursor-pointer items-center justify-between gap-2 rounded-md px-1 py-2.5 text-left text-xs font-medium text-foreground transition-colors hover:text-foreground/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50">
        {title}
        <ChevronDown
          className="size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 group-data-open:rotate-180"
          aria-hidden
        />
      </CollapsibleTrigger>
      <CollapsibleContent className="pb-3">{children}</CollapsibleContent>
    </Collapsible>
  );
}

export function BotContextRail({
  bot,
  activeRun,
  showConnectionSettings,
  onBotSaved,
  onCollapse,
  className,
}: BotContextRailProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsRef = useRef<HTMLDivElement>(null);

  function handleOpenSettings() {
    setSettingsOpen(true);
    requestAnimationFrame(() => {
      settingsRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  }

  return (
    <div className={cn("flex min-h-0 flex-1 flex-col", className)}>
      <div className="flex h-11 shrink-0 items-center justify-end gap-0.5 px-2">
        {bot ? (
          <ComposerIconButton label="Bot settings" className="size-7" onClick={handleOpenSettings}>
            <Settings2 className="size-4" aria-hidden />
          </ComposerIconButton>
        ) : null}
        {onCollapse ? (
          <ComposerIconButton label="Hide details" className="size-7" onClick={onCollapse}>
            <ChevronsRight className="size-4" aria-hidden />
          </ComposerIconButton>
        ) : null}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        <ComputerStatePanel bot={bot} activeRun={activeRun} variant="minimal" />

        {bot ? (
          <div className="mt-2">
            <RailSection title="Routines" defaultOpen>
              <BotRoutinesSidebar botId={bot.id} variant="minimal" />
            </RailSection>
            {bot.computerId ? (
              <RailSection title="Files">
                <ComputerWorkspaceTree computerId={bot.computerId} className="px-0.5" />
              </RailSection>
            ) : null}
            <RailSection title="Memory">
              <BotContext botId={bot.id} embedded />
            </RailSection>
            <div ref={settingsRef} className="scroll-mt-2">
              <RailSection title="Settings" open={settingsOpen} onOpenChange={setSettingsOpen}>
                <BotSettings bot={bot} onSaved={onBotSaved} embedded />
              </RailSection>
            </div>
          </div>
        ) : null}

        {showConnectionSettings ? (
          <div className="mt-3 border-t border-border pt-3">
            <ProviderStatusCard />
          </div>
        ) : null}
      </div>
    </div>
  );
}
