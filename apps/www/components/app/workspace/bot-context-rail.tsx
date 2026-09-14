"use client";

import type { BotSummary, RunSummary } from "@/lib/api-types";
import { BotContext } from "@/components/app/bot-context";
import { BotSettings } from "@/components/app/bot-settings";
import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { BotRoutinesSidebar } from "./bot-routines-sidebar";
import { ComputerStatePanel } from "./computer-state-panel";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { cn } from "cn";
import { ChevronDown } from "@/components/icons/lucide";
import type { ReactNode } from "react";

interface BotContextRailProps {
  bot: BotSummary | null;
  activeRun: RunSummary | null;
  showConnectionSettings: boolean;
  onBotSaved: (bot: BotSummary) => void;
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
      className="group border-b border-border/60 last:border-b-0"
    >
      <CollapsibleTrigger className="flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg px-1 py-3 text-left text-sm font-semibold transition-colors hover:bg-white/60">
        {title}
        <ChevronDown
          className="size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-data-open:rotate-180"
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
  className,
}: BotContextRailProps) {
  return (
    <div className={cn("flex min-h-0 flex-1 flex-col", className)}>
      <div className="flex h-12 shrink-0 items-center border-b border-border/60 px-4">
        <h2 className="truncate text-sm font-semibold tracking-tight">
          {bot?.name ? `${bot.name}` : "Bot details"}
        </h2>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        <ComputerStatePanel bot={bot} activeRun={activeRun} variant="minimal" />

        {bot ? (
          <div className="mt-1">
            <RailSection title="Routines">
              <BotRoutinesSidebar botId={bot.id} variant="minimal" />
            </RailSection>
            <RailSection title="Memory">
              <BotContext botId={bot.id} embedded />
            </RailSection>
            <RailSection title="Settings">
              <BotSettings bot={bot} onSaved={onBotSaved} embedded />
            </RailSection>
          </div>
        ) : null}

        {showConnectionSettings ? (
          <div className="mt-3 border-t border-border/60 pt-3">
            <ProviderStatusCard />
          </div>
        ) : null}
      </div>
    </div>
  );
}
