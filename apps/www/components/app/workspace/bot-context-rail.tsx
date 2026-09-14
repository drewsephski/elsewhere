"use client";

import type { BotSummary, RunSummary } from "@/lib/api-types";
import { BotContext } from "@/components/app/bot-context";
import { BotSettings } from "@/components/app/bot-settings";
import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { BotRoutinesSidebar } from "./bot-routines-sidebar";
import { ComputerStatePanel } from "./computer-state-panel";
import { cn } from "cn";

interface BotContextRailProps {
  bot: BotSummary | null;
  activeRun: RunSummary | null;
  showConnectionSettings: boolean;
  onBotSaved: (bot: BotSummary) => void;
  className?: string;
}

export function BotContextRail({
  bot,
  activeRun,
  showConnectionSettings,
  onBotSaved,
  className,
}: BotContextRailProps) {
  return (
    <div className={cn("flex min-h-0 flex-col gap-3 overflow-y-auto p-3", className)}>
      <ComputerStatePanel bot={bot} activeRun={activeRun} />
      {bot ? <BotRoutinesSidebar botId={bot.id} /> : null}
      {showConnectionSettings ? <ProviderStatusCard /> : null}
      {bot ? (
        <div className="workspace-rail-card [&_.surface-card]:border-0 [&_.surface-card]:bg-transparent [&_.surface-card]:p-0 [&_.surface-card]:shadow-none">
          <h3 className="mb-2 text-sm font-semibold">Memory</h3>
          <BotContext botId={bot.id} />
        </div>
      ) : null}
      {bot ? (
        <div className="workspace-rail-card [&_details]:border-0 [&_details]:bg-transparent [&_details]:p-0 [&_details]:shadow-none">
          <BotSettings bot={bot} onSaved={onBotSaved} />
        </div>
      ) : null}
    </div>
  );
}
