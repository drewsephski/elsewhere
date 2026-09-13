import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { Bot } from "@/lib/definitions";
import {
  getBotActivityIndicator,
  getBotSubtitle,
  type BotActivityIndicator,
} from "@/lib/bot-visual";
import { BotCreatureAvatar } from "@/ui/bot-creature-avatar";
import { isDemoAgent, sortBotsForNav } from "@/lib/demo-agent";
import {
  readWorkspaceName,
  writeWorkspaceName,
} from "@/lib/workspace-preferences";
import { InlineRenameLabel } from "@/ui/inline-rename-label";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import {
  Activity,
  ChevronDown,
  FolderOpen,
  MessageSquarePlus,
  PanelLeftClose,
  Plus,
  Search,
  Settings2,
  Sparkles,
} from "@/components/icons/lucide";

interface BotNavProps {
  bots: Bot[];
  selectedBotId: string | null;
  onSelectBot: (id: string) => void;
  onCreateBot: () => void;
  onOpenSettings: () => void;
  apiKeyConfigured?: boolean;
  isStreaming?: boolean;
  className?: string;
  onNavigate?: () => void;
  collapsed?: boolean;
  onToggleCollapsed?: () => void;
  onRenameBot?: (id: string, name: string) => void | Promise<void>;
}

function NavIconButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick?: () => void;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-8 shrink-0 rounded-lg text-muted-foreground hover:bg-white/80"
            onClick={onClick}
            disabled={disabled}
            aria-label={label}
          />
        }
      >
        {children}
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

interface SidebarUtilityLinkProps {
  icon: ReactNode;
  label: string;
  onClick?: () => void;
  disabled?: boolean;
}

function SidebarUtilityLink({
  icon,
  label,
  onClick,
  disabled,
}: SidebarUtilityLinkProps) {
  return (
    <Button
      type="button"
      variant="ghost"
      disabled={disabled}
      onClick={onClick}
      className="h-7 w-full justify-start gap-2 rounded-lg px-2 text-[11px] font-normal text-muted-foreground hover:bg-white/70 hover:text-foreground"
    >
      <span className="text-muted-foreground/80 [&_svg]:size-3.5" aria-hidden>
        {icon}
      </span>
      {label}
    </Button>
  );
}

function BotActivityDot({ status }: { status: BotActivityIndicator }) {
  if (!status) {
    return <span className="mt-2 size-2 shrink-0" aria-hidden />;
  }

  return (
    <span
      className={cn(
        "mt-2 size-2 shrink-0 rounded-full",
        status === "streaming" && "bg-sky-500 animate-pulse",
        status === "active" && "bg-emerald-500",
        status === "recent" && "bg-sky-400/90",
      )}
      aria-hidden
    />
  );
}

interface BotNavRowProps {
  bot: Bot;
  selected: boolean;
  streaming: boolean;
  onSelect: () => void;
  onRename?: (name: string) => void;
}

function BotNavRow({
  bot,
  selected,
  streaming,
  onSelect,
  onRename,
}: BotNavRowProps) {
  const activity = getBotActivityIndicator(bot, selected, streaming);
  const demo = isDemoAgent(bot);

  return (
    <div
      className={cn(
        "group relative flex w-full items-start gap-2.5 rounded-lg px-2 py-1.5 text-left transition-colors",
        selected
          ? "bg-white shadow-sm ring-1 ring-black/[0.04]"
          : "hover:bg-white/65",
      )}
    >
      {selected && (
        <span
          className="absolute top-1/2 left-0 h-6 w-[3px] -translate-y-1/2 rounded-full bg-foreground/80"
          aria-hidden
        />
      )}
      <button
        type="button"
        onClick={onSelect}
        aria-current={selected ? "true" : undefined}
        className="flex min-w-0 flex-1 items-start gap-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-foreground/15 focus-visible:ring-offset-1 focus-visible:ring-offset-transparent rounded-md"
      >
        <BotCreatureAvatar
          name={bot.name}
          size="md"
          className="mt-0.5 shadow-sm"
          animated={selected && streaming}
        />
        <span className="min-w-0 flex-1 pt-0.5">
          <span className="flex min-w-0 items-center gap-1.5">
            {onRename ? (
              <InlineRenameLabel
                value={bot.name}
                onCommit={onRename}
                ariaLabel={`Rename ${bot.name}`}
                className="text-[12px] font-medium text-foreground"
                inputClassName="text-[12px]"
              />
            ) : (
              <span className="truncate text-[12px] font-medium text-foreground">
                {bot.name}
              </span>
            )}
            {demo && (
              <Badge
                variant="secondary"
                className="h-4 shrink-0 px-1.5 text-[9px] font-medium tracking-wide text-muted-foreground"
              >
                Demo
              </Badge>
            )}
          </span>
          <span className="mt-0.5 line-clamp-1 text-[11px] text-muted-foreground">
            {getBotSubtitle(bot)}
          </span>
        </span>
      </button>
      <BotActivityDot status={activity} />
    </div>
  );
}

export function BotNav({
  bots,
  selectedBotId,
  onSelectBot,
  onCreateBot,
  onOpenSettings,
  apiKeyConfigured = false,
  isStreaming = false,
  className,
  onNavigate,
  collapsed = false,
  onToggleCollapsed,
  onRenameBot,
}: BotNavProps) {
  const [query, setQuery] = useState("");
  const [workspaceName, setWorkspaceName] = useState(readWorkspaceName);
  const searchInputRef = useRef<HTMLInputElement>(null);

  function handleWorkspaceRename(next: string) {
    setWorkspaceName(next);
    writeWorkspaceName(next);
  }

  const sortedBots = useMemo(() => sortBotsForNav(bots), [bots]);

  const filteredBots = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) {
      return sortedBots;
    }
    return sortedBots.filter(
      (bot) =>
        bot.name.toLowerCase().includes(q) ||
        bot.model.toLowerCase().includes(q) ||
        bot.systemPrompt.toLowerCase().includes(q) ||
        (bot.description?.toLowerCase().includes(q) ?? false),
    );
  }, [sortedBots, query]);

  function handleSelect(id: string) {
    onSelectBot(id);
    onNavigate?.();
  }

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchInputRef.current?.focus();
      }
      if (event.key === "Escape" && document.activeElement === searchInputRef.current) {
        searchInputRef.current?.blur();
        setQuery("");
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const collapsedActiveBot = useMemo(() => {
    if (!collapsed) {
      return null;
    }
    if (selectedBotId) {
      return bots.find((bot) => bot.id === selectedBotId) ?? null;
    }
    return sortedBots[0] ?? null;
  }, [bots, collapsed, selectedBotId, sortedBots]);

  const collapsedRailBots = useMemo(() => {
    if (!collapsed || !collapsedActiveBot) {
      return sortedBots.length > 1 ? sortedBots : [];
    }
    return sortedBots.filter((bot) => bot.id !== collapsedActiveBot.id);
  }, [sortedBots, collapsed, collapsedActiveBot]);

  if (collapsed) {
    return (
      <div
        className={cn(
          "flex h-full min-h-0 w-full flex-col items-center px-1 pb-2",
          className,
        )}
      >
        <div className="flex w-full shrink-0 flex-col items-center gap-2 pt-3">
          {collapsedActiveBot ? (
            <Tooltip>
              <TooltipTrigger
                render={
                  <button
                    type="button"
                    onClick={() => handleSelect(collapsedActiveBot.id)}
                    className="rounded-full ring-2 ring-foreground/10 ring-offset-2 ring-offset-[#f4f4f5]"
                    aria-label={`${collapsedActiveBot.name}, current assistant`}
                    aria-current="true"
                  />
                }
              >
                <BotCreatureAvatar
                  name={collapsedActiveBot.name}
                  size="lg"
                  className="size-9 shadow-sm"
                  animated={isStreaming}
                />
              </TooltipTrigger>
              <TooltipContent side="right">{collapsedActiveBot.name}</TooltipContent>
            </Tooltip>
          ) : (
            <Tooltip>
              <TooltipTrigger
                render={
                  <button
                    type="button"
                    onClick={onCreateBot}
                    className="flex size-9 items-center justify-center rounded-full border border-dashed border-border/80 bg-white/60 text-muted-foreground shadow-sm hover:bg-white"
                    aria-label="Create your first assistant"
                  />
                }
              >
                <Plus className="size-4" aria-hidden />
              </TooltipTrigger>
              <TooltipContent side="right">New assistant</TooltipContent>
            </Tooltip>
          )}
        </div>

        <div className="min-h-0 w-full flex-1">
          {collapsedRailBots.length > 0 ? (
            <ScrollArea className="h-full max-h-full w-full">
              <nav
                aria-label="Other assistants"
                className="flex flex-col items-center gap-1.5 py-2"
              >
                {collapsedRailBots.map((bot) => (
                  <Tooltip key={bot.id}>
                    <TooltipTrigger
                      render={
                        <button
                          type="button"
                          onClick={() => handleSelect(bot.id)}
                          className="rounded-full transition-transform hover:scale-105"
                          aria-label={bot.name}
                        />
                      }
                    >
                      <BotCreatureAvatar
                        name={bot.name}
                        size="md"
                        className="shadow-sm ring-border/40"
                      />
                    </TooltipTrigger>
                    <TooltipContent side="right">{bot.name}</TooltipContent>
                  </Tooltip>
                ))}
              </nav>
            </ScrollArea>
          ) : null}
        </div>

        <div className="flex w-full shrink-0 flex-col items-center gap-0.5 border-t border-border/50 pt-2">
          <NavIconButton label="New assistant" onClick={onCreateBot}>
            <Plus className="size-4" aria-hidden />
          </NavIconButton>
          <NavIconButton label="Settings" onClick={onOpenSettings}>
            <Settings2 className="size-4" aria-hidden />
          </NavIconButton>
        </div>
      </div>
    );
  }

  return (
    <div className={cn("flex h-full min-h-0 flex-col bg-[#f4f4f5]", className)}>
      <div className="flex shrink-0 items-center justify-between gap-2 px-4 pt-4 pb-1">
        <span className="text-[15px] font-semibold tracking-tight text-foreground">
          GPTBot
        </span>
        <div className="flex items-center gap-0.5">
          <TooltipProvider delay={300}>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-8 rounded-lg text-muted-foreground hover:bg-white/80"
                    onClick={onCreateBot}
                    aria-label="New assistant"
                  />
                }
              >
                <Plus className="size-4" aria-hidden />
              </TooltipTrigger>
              <TooltipContent side="bottom">New assistant</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-8 rounded-lg text-muted-foreground hover:bg-white/80"
                    onClick={onToggleCollapsed}
                    aria-label="Collapse sidebar"
                  />
                }
              >
                <PanelLeftClose className="size-4" aria-hidden />
              </TooltipTrigger>
              <TooltipContent side="bottom">Collapse sidebar</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </div>

      <div className="shrink-0 px-3 pb-2 pt-1">
        <div className="relative">
          <Search
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground/70"
            aria-hidden
          />
          <Input
            ref={searchInputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search agents"
            className="h-9 rounded-xl border-0 bg-white/90 pr-14 pl-9 shadow-none ring-1 ring-border/40 placeholder:text-muted-foreground/70 focus-visible:ring-foreground/15"
            aria-label="Search agents"
          />
          <kbd
            className="pointer-events-none absolute top-1/2 right-2.5 hidden -translate-y-1/2 rounded-md border border-border/50 bg-[#f4f4f5] px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground sm:inline"
          >
            ⌘K
          </kbd>
        </div>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <div className="px-2 pb-2">
          <div className="mb-2 flex items-center justify-between gap-2 px-1 pt-0.5">
            <div className="flex min-w-0 items-center gap-2">
              <span className="text-[11px] font-semibold tracking-tight text-foreground/90">
                Agents
              </span>
            </div>
            <span
              className="shrink-0 rounded-md bg-white/90 px-1.5 py-0.5 text-[10px] tabular-nums text-muted-foreground ring-1 ring-border/35"
            >
              {bots.length}
            </span>
          </div>

          <nav aria-label="Agents" className="space-y-0.5">
            {filteredBots.length === 0 ? (
              <div className="mx-0.5 rounded-xl border border-dashed border-border/55 bg-white/45 px-3 py-5 text-center">
                {bots.length === 0 ? (
                  <>
                    <div
                      className="mx-auto mb-2.5 flex size-9 items-center justify-center rounded-full bg-white shadow-sm ring-1 ring-border/40"
                      aria-hidden
                    >
                      <Sparkles className="size-4 text-muted-foreground/90" />
                    </div>
                    <p className="text-[12px] font-medium text-foreground">
                      No agents yet
                    </p>
                    <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">
                      Create one or chat with Scout to explore.
                    </p>
                    <Button
                      type="button"
                      size="sm"
                      className="mt-3 h-8 gap-1.5 rounded-lg px-3 text-[11px] font-medium"
                      onClick={onCreateBot}
                    >
                      <Plus className="size-3.5" aria-hidden />
                      New assistant
                    </Button>
                  </>
                ) : (
                  <p className="text-[11px] leading-relaxed text-muted-foreground">
                    No matches for your search.
                  </p>
                )}
              </div>
            ) : (
              filteredBots.map((bot) => (
                <BotNavRow
                  key={bot.id}
                  bot={bot}
                  selected={bot.id === selectedBotId}
                  streaming={bot.id === selectedBotId && isStreaming}
                  onSelect={() => handleSelect(bot.id)}
                  onRename={
                    onRenameBot
                      ? (name) => {
                          void onRenameBot(bot.id, name);
                        }
                      : undefined
                  }
                />
              ))
            )}
          </nav>

          <div className="mt-3 px-0.5">
            <div className="rounded-xl border border-border/45 bg-white/55 p-1 shadow-sm ring-1 ring-black/[0.02]">
              <Collapsible defaultOpen className="group">
                <CollapsibleTrigger
                  className="flex w-full items-center gap-1.5 rounded-lg px-1.5 py-1.5 text-left text-[11px] font-medium text-foreground/85 transition-colors hover:bg-white/70"
                >
                  <FolderOpen
                    className="size-3.5 shrink-0 text-muted-foreground/80"
                    aria-hidden
                  />
                  <InlineRenameLabel
                    value={workspaceName}
                    onCommit={handleWorkspaceRename}
                    ariaLabel="Workspace name"
                    className="min-w-0 flex-1 text-[11px] font-medium text-foreground/90"
                    inputClassName="text-[11px]"
                  />
                  <span
                    className="shrink-0 rounded bg-[#f4f4f5] px-1.5 py-0.5 text-[10px] tabular-nums text-muted-foreground"
                  >
                    {bots.length}
                  </span>
                  <ChevronDown
                    className="size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 group-data-open:rotate-180"
                    aria-hidden
                  />
                </CollapsibleTrigger>
                <CollapsibleContent className="space-y-0 px-0.5 pb-0.5 pt-0.5">
                  <Button
                    type="button"
                    variant="ghost"
                    disabled
                    className="h-7 w-full justify-start gap-1.5 rounded-md px-1.5 text-[10px] font-normal text-muted-foreground"
                  >
                    <Plus className="size-3" aria-hidden />
                    New section
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    disabled
                    className="h-7 w-full justify-start gap-1.5 rounded-md px-1.5 text-[10px] font-normal text-muted-foreground"
                  >
                    <MessageSquarePlus className="size-3" aria-hidden />
                    New group chat
                  </Button>
                </CollapsibleContent>
              </Collapsible>
            </div>
          </div>
        </div>
      </ScrollArea>

      <div className="shrink-0 space-y-0.5 border-t border-border/40 px-2 py-1.5">
        <SidebarUtilityLink icon={<Activity />} label="Activity" disabled />
        <SidebarUtilityLink icon={<Sparkles />} label="Skills" disabled />
      </div>

      <div className="shrink-0 border-t border-border/50 p-2">
        <button
          type="button"
          onClick={onOpenSettings}
          className="flex w-full items-center gap-2 rounded-lg px-1.5 py-1 text-left transition-colors hover:bg-white/60"
        >
          <Avatar size="sm" className="size-7 bg-white ring-1 ring-border/40">
            <AvatarFallback className="bg-white text-[10px] font-medium text-foreground">
              Y
            </AvatarFallback>
          </Avatar>
          <div className="min-w-0 flex-1">
            <p className="truncate text-[12px] font-medium text-foreground">
              {workspaceName}
            </p>
            <p
              className={cn(
                "truncate text-[10px]",
                apiKeyConfigured
                  ? "text-emerald-700/90"
                  : "text-muted-foreground",
              )}
            >
              {apiKeyConfigured ? "OpenAI connected" : "Connect API key"}
            </p>
          </div>
          <Settings2
            className="size-4 shrink-0 text-muted-foreground"
            aria-hidden
          />
        </button>
      </div>
    </div>
  );
}
