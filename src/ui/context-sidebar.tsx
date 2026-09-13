import type { Bot, ModelDescriptor } from "@/lib/definitions";
import { getBotInitials, getBotAvatarClass } from "@/lib/bot-visual";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { BotSettingsPanel } from "@/ui/bot-settings-panel";
import { VmDiagnosticsPanel } from "@/ui/vm-diagnostics-panel";
import { cn } from "@/lib/utils";
import {
  Activity,
  ChevronDown,
  Circle,
  Monitor,
  Pause,
  Plus,
} from "lucide-react";

interface RoutineItem {
  id: string;
  title: string;
  schedule: string;
  status: "active" | "paused";
}

const PLACEHOLDER_ROUTINES: RoutineItem[] = [
  {
    id: "vm-health",
    title: "Agent computer health check",
    schedule: "Every 30 minutes",
    status: "active",
  },
  {
    id: "workspace-sync",
    title: "Workspace sync",
    schedule: "Weekdays at 6:15 AM",
    status: "paused",
  },
];

interface ContextSidebarProps {
  bot: Bot | null;
  displayBot: Bot | null;
  models: ModelDescriptor[];
  modelsLoading: boolean;
  modelsError: string | null;
  onBotChange: (patch: Partial<Pick<Bot, "name" | "systemPrompt" | "model">>) => void;
  onSaveBot: () => void;
  onArchiveBot: () => void;
  botSaving: boolean;
  className?: string;
}

export function ContextSidebar({
  bot,
  displayBot,
  models,
  modelsLoading,
  modelsError,
  onBotChange,
  onSaveBot,
  onArchiveBot,
  botSaving,
  className,
}: ContextSidebarProps) {
  const screenLabel = bot ? `${bot.name}'s screen` : "Agent screen";

  return (
    <aside
      className={cn(
        "hidden h-full w-[min(100%,17.5rem)] shrink-0 flex-col border-l border-border/80 bg-[#fafafa] lg:flex xl:w-80",
        className,
      )}
      aria-label="Context panel"
    >
      <ScrollArea className="h-full flex-1">
        <div className="flex flex-col gap-4 p-4">
          <Card className="overflow-hidden border-border/70 bg-white shadow-sm">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 px-3 py-2.5">
              <CardTitle className="text-xs font-medium text-muted-foreground">
                {screenLabel}
              </CardTitle>
              <Monitor className="size-3.5 text-muted-foreground" aria-hidden />
            </CardHeader>
            <CardContent className="p-2 pt-0">
              <div
                className="relative aspect-[4/3] overflow-hidden rounded-lg border border-border/60 bg-gradient-to-br from-slate-100 via-white to-slate-50"
                role="img"
                aria-label="Agent workspace preview"
              >
                <div className="absolute inset-2 rounded-md border border-border/40 bg-white/90 p-2 shadow-inner">
                  <div className="mb-2 flex gap-1">
                    <span className="size-2 rounded-full bg-red-400/80" />
                    <span className="size-2 rounded-full bg-amber-400/80" />
                    <span className="size-2 rounded-full bg-emerald-400/80" />
                  </div>
                  <div className="space-y-1.5">
                    <div className="h-1.5 w-3/4 rounded-full bg-muted" />
                    <div className="h-1.5 w-full rounded-full bg-muted/80" />
                    <div className="h-1.5 w-5/6 rounded-full bg-muted/60" />
                    <div className="mt-2 grid grid-cols-3 gap-1">
                      <div className="aspect-square rounded bg-muted/70" />
                      <div className="aspect-square rounded bg-muted/50" />
                      <div className="aspect-square rounded bg-muted/40" />
                    </div>
                  </div>
                </div>
                {bot && (
                  <div
                    className={cn(
                      "absolute bottom-2 left-2 flex size-8 items-center justify-center rounded-lg text-[10px] font-semibold shadow-sm",
                      getBotAvatarClass(bot.name),
                    )}
                  >
                    {getBotInitials(bot.name)}
                  </div>
                )}
              </div>
            </CardContent>
          </Card>

          <section aria-labelledby="routines-heading">
            <div className="mb-2 flex items-center justify-between gap-2">
              <h2
                id="routines-heading"
                className="text-sm font-semibold tracking-tight text-foreground"
              >
                Routines
              </h2>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="text-muted-foreground"
                aria-label="Add routine"
                disabled
              >
                <Plus className="size-4" />
              </Button>
            </div>
            <ul className="space-y-1">
              {PLACEHOLDER_ROUTINES.map((routine) => (
                <li key={routine.id}>
                  <button
                    type="button"
                    className="flex w-full items-start gap-2.5 rounded-xl px-2 py-2 text-left transition-colors hover:bg-white/80"
                    disabled
                  >
                    <span className="mt-0.5 text-muted-foreground" aria-hidden>
                      {routine.status === "active" ? (
                        <Activity className="size-3.5 text-emerald-600" />
                      ) : (
                        <Pause className="size-3.5" />
                      )}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium text-foreground">
                        {routine.title}
                      </span>
                      <span className="mt-0.5 flex items-center gap-1.5 text-[11px] text-muted-foreground">
                        <Circle className="size-1 fill-current" />
                        {routine.schedule}
                        {routine.status === "paused" && (
                          <span className="text-amber-700/80">· Paused</span>
                        )}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </section>

          <Separator className="bg-border/60" />

          <Collapsible defaultOpen={false}>
            <CollapsibleTrigger
              className="flex w-full items-center justify-between rounded-lg px-1 py-1.5 text-sm font-medium text-foreground hover:bg-white/60"
            >
              Agent computer
              <ChevronDown className="size-4 text-muted-foreground [[data-state=open]_&]:rotate-180" />
            </CollapsibleTrigger>
            <CollapsibleContent className="pt-2">
              <VmDiagnosticsPanel open={true} embedded />
            </CollapsibleContent>
          </Collapsible>

          {displayBot && (
            <>
              <Separator className="bg-border/60" />
              <BotSettingsPanel
                bot={displayBot}
                models={models}
                modelsLoading={modelsLoading}
                modelsError={modelsError}
                onChange={onBotChange}
                onSave={onSaveBot}
                onArchive={onArchiveBot}
                saving={botSaving}
                variant="sidebar"
              />
            </>
          )}
        </div>
      </ScrollArea>
    </aside>
  );
}
