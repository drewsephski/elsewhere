import { useCallback, useEffect, useState, type ReactNode } from "react";
import type { Bot, ModelDescriptor } from "@/lib/definitions";
import { BotCreatureAvatar } from "@/ui/bot-creature-avatar";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ScrollArea } from "@/components/ui/scroll-area";
import { BotSettingsPanel } from "@/ui/bot-settings-panel";
import { InlineRenameLabel } from "@/ui/inline-rename-label";
import { VmDiagnosticsPanel } from "@/ui/vm-diagnostics-panel";
import { ContextSidebarExpandTab } from "@/ui/context-sidebar-expand-tab";
import { TooltipProvider } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import {
  Activity,
  ChevronDown,
  Circle,
  Link2,
  Monitor,
  PanelRightClose,
  Pause,
  Plus,
  Wand2,
} from "@/components/icons/lucide";

const CONTEXT_SIDEBAR_COLLAPSED_KEY = "gptbot.context-sidebar.collapsed";

function readContextSidebarCollapsed(): boolean {
  try {
    return localStorage.getItem(CONTEXT_SIDEBAR_COLLAPSED_KEY) === "true";
  } catch {
    return false;
  }
}

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

function ContextSectionHeader({
  id,
  title,
  icon,
  trailing,
}: {
  id?: string;
  title: string;
  icon?: ReactNode;
  trailing?: ReactNode;
}) {
  return (
    <div className="mb-1.5 flex items-center justify-between gap-2 px-0.5">
      <div className="flex min-w-0 items-center gap-2">
        <h2
          id={id}
          className="flex min-w-0 items-center gap-1.5 text-[11px] font-semibold tracking-tight text-foreground/90"
        >
          {icon}
          {title}
        </h2>
      </div>
      {trailing}
    </div>
  );
}

function ContextPanelCard({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "rounded-xl border border-border/45 bg-white/55 p-1 shadow-sm ring-1 ring-black/[0.02]",
        className,
      )}
    >
      {children}
    </div>
  );
}

interface ContextSidebarProps {
  bot: Bot | null;
  displayBot: Bot | null;
  models: ModelDescriptor[];
  modelsLoading: boolean;
  modelsError: string | null;
  onBotChange: (patch: Partial<Pick<Bot, "name" | "systemPrompt" | "model">>) => void;
  onSaveBot: () => void;
  onArchiveBot: () => void;
  onRenameBot?: (id: string, name: string) => void | Promise<void>;
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
  onRenameBot,
  botSaving,
  className,
}: ContextSidebarProps) {
  const [collapsed, setCollapsed] = useState(readContextSidebarCollapsed);
  const nameSource = displayBot ?? bot;

  useEffect(() => {
    try {
      localStorage.setItem(CONTEXT_SIDEBAR_COLLAPSED_KEY, String(collapsed));
    } catch {
      /* ignore */
    }
  }, [collapsed]);

  const handleCollapse = useCallback(() => {
    setCollapsed(true);
  }, []);

  const handleExpand = useCallback(() => {
    setCollapsed(false);
  }, []);

  function handleRenameAgent(name: string) {
    if (!nameSource) {
      return;
    }
    if (onRenameBot) {
      void onRenameBot(nameSource.id, name);
      return;
    }
    onBotChange({ name });
    void onSaveBot();
  }

  if (collapsed) {
    return (
      <aside
        className={cn(
          "relative hidden h-full w-0 shrink-0 overflow-visible lg:flex",
          className,
        )}
        aria-label="Context panel"
        data-collapsed=""
      >
        <TooltipProvider delay={300}>
          <ContextSidebarExpandTab onExpand={handleExpand} />
        </TooltipProvider>
      </aside>
    );
  }

  return (
    <aside
      className={cn(
        "relative hidden h-full w-[min(100%,17.5rem)] shrink-0 flex-col border-l border-border/50 bg-[#f4f4f5] lg:flex xl:w-80",
        className,
      )}
      aria-label="Context panel"
    >
      <div className="flex shrink-0 items-center justify-between gap-2 border-b border-border/40 bg-white/60 px-2 py-1.5">
        <span className="text-[10px] font-semibold tracking-tight text-foreground/80">
          Context
        </span>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="size-7 text-muted-foreground"
          onClick={handleCollapse}
          aria-label="Close context panel"
        >
          <PanelRightClose className="size-3.5" aria-hidden />
        </Button>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="flex flex-col gap-2 p-2">
          <ContextPanelCard className="overflow-hidden p-0">
            <div className="flex items-center justify-between gap-2 border-b border-border/35 bg-white/70 px-3 py-2.5">
              <div className="flex min-w-0 items-center gap-0.5 text-[11px] font-semibold tracking-tight text-foreground/90">
                {nameSource ? (
                  <>
                    <InlineRenameLabel
                      value={nameSource.name}
                      onCommit={handleRenameAgent}
                      ariaLabel={`Rename ${nameSource.name}`}
                      className="max-w-[9rem] text-[11px] font-semibold text-foreground/90"
                      inputClassName="text-[11px]"
                    />
                    <span className="shrink-0 font-medium text-muted-foreground">
                      &apos;s screen
                    </span>
                  </>
                ) : (
                  <span>Agent screen</span>
                )}
              </div>
              <Monitor
                className="size-3.5 shrink-0 text-muted-foreground/80"
                aria-hidden
              />
            </div>
            <div className="p-2">
              <div
                className="relative aspect-[4/3] overflow-hidden rounded-lg border border-border/50 bg-gradient-to-br from-zinc-100 via-white to-zinc-50"
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
                  <BotCreatureAvatar
                    name={bot.name}
                    size="md"
                    className="absolute bottom-2 left-2 shadow-sm"
                  />
                )}
              </div>
            </div>
          </ContextPanelCard>

          <ContextPanelCard>
            <nav
              aria-label="Agent tools"
              className="flex flex-col gap-0.5 p-0.5"
            >
              <Button
                type="button"
                variant="ghost"
                disabled
                className="h-8 w-full min-w-0 justify-start gap-2 rounded-lg px-2.5 text-[11px] font-normal text-muted-foreground hover:bg-white/80"
              >
                <Link2 className="size-3.5 shrink-0" aria-hidden />
                <span className="truncate">Connectors</span>
              </Button>
              <Button
                type="button"
                variant="ghost"
                disabled
                className="h-8 w-full min-w-0 justify-start gap-2 rounded-lg px-2.5 text-[11px] font-normal text-muted-foreground hover:bg-white/80"
              >
                <Monitor className="size-3.5 shrink-0" aria-hidden />
                <span className="truncate" title="Shared computer">
                  Shared computer
                </span>
              </Button>
            </nav>
          </ContextPanelCard>

          <section aria-labelledby="routines-heading">
            <ContextSectionHeader
              id="routines-heading"
              title="Routines"
              icon={
                <Wand2
                  className="size-3.5 text-muted-foreground/80"
                  aria-hidden
                />
              }
              trailing={
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  className="size-7 text-muted-foreground"
                  aria-label="Add routine"
                  disabled
                >
                  <Plus className="size-3.5" />
                </Button>
              }
            />
            <ContextPanelCard className="p-0.5">
              <ul className="space-y-0.5">
                {PLACEHOLDER_ROUTINES.map((routine) => (
                  <li key={routine.id}>
                    <button
                      type="button"
                      className="flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left transition-colors hover:bg-white/75"
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
                        <span className="block truncate text-[12px] font-medium text-foreground">
                          {routine.title}
                        </span>
                        <span className="mt-0.5 flex items-center gap-1.5 text-[10px] text-muted-foreground">
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
            </ContextPanelCard>
          </section>

          <ContextPanelCard className="space-y-0 p-0.5">
            <Collapsible defaultOpen={false} className="group">
              <CollapsibleTrigger
                className="flex w-full items-center gap-2 rounded-lg px-1.5 py-1.5 text-left transition-colors hover:bg-white/70"
              >
                <Monitor
                  className="size-3.5 shrink-0 text-muted-foreground/80"
                  aria-hidden
                />
                <span className="min-w-0 flex-1 text-[11px] font-semibold tracking-tight text-foreground/90">
                  Agent computer
                </span>
                <ChevronDown
                  className="size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 group-data-open:rotate-180"
                  aria-hidden
                />
              </CollapsibleTrigger>
              <CollapsibleContent className="px-1 pb-0.5 pt-0.5">
                <VmDiagnosticsPanel open={true} embedded />
              </CollapsibleContent>
            </Collapsible>

            {displayBot && (
              <BotSettingsPanel
                bot={displayBot}
                models={models}
                modelsLoading={modelsLoading}
                modelsError={modelsError}
                onChange={onBotChange}
                onSave={onSaveBot}
                onArchive={onArchiveBot}
                onRename={
                  onRenameBot
                    ? (name) => {
                        void onRenameBot(displayBot.id, name);
                      }
                    : handleRenameAgent
                }
                saving={botSaving}
                variant="sidebar"
              />
            )}
          </ContextPanelCard>
        </div>
      </ScrollArea>
    </aside>
  );
}
