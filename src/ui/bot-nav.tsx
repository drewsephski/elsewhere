import { useMemo, useState } from "react";
import type { Bot } from "@/lib/definitions";
import {
  getBotAvatarClass,
  getBotInitials,
  getBotPreview,
} from "@/lib/bot-visual";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import {
  ChevronDown,
  Plus,
  Search,
  Settings2,
  Store,
  Users,
} from "lucide-react";

interface BotNavProps {
  bots: Bot[];
  selectedBotId: string | null;
  onSelectBot: (id: string) => void;
  onCreateBot: () => void;
  onOpenSettings: () => void;
  className?: string;
  onNavigate?: () => void;
}

function formatRelativeTime(timestamp: number): string {
  const diff = Date.now() - timestamp;
  const dayMs = 86_400_000;
  if (diff < dayMs) {
    return "Today";
  }
  if (diff < dayMs * 2) {
    return "Yesterday";
  }
  const date = new Date(timestamp);
  return date.toLocaleDateString(undefined, { month: "numeric", day: "numeric" });
}

export function BotNav({
  bots,
  selectedBotId,
  onSelectBot,
  onCreateBot,
  onOpenSettings,
  className,
  onNavigate,
}: BotNavProps) {
  const [query, setQuery] = useState("");

  const filteredBots = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) {
      return bots;
    }
    return bots.filter(
      (bot) =>
        bot.name.toLowerCase().includes(q) ||
        bot.model.toLowerCase().includes(q) ||
        bot.systemPrompt.toLowerCase().includes(q),
    );
  }, [bots, query]);

  const profileBots = bots.slice(0, 2);

  function handleSelect(id: string) {
    onSelectBot(id);
    onNavigate?.();
  }

  return (
    <div className={cn("flex h-full flex-col bg-[#f5f5f7]", className)}>
      <div className="px-3 pt-3 pb-2">
        <div className="relative">
          <Search
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search"
            className="h-9 rounded-xl border-border/60 bg-white pl-9 shadow-none"
            aria-label="Search assistants"
          />
        </div>
      </div>

      {profileBots.length > 0 && (
        <div className="px-3 pb-2">
          <p className="mb-2 px-1 text-[11px] font-medium tracking-wide text-muted-foreground uppercase">
            Profiles
          </p>
          <div className="flex gap-2">
            {profileBots.map((bot) => {
              const selected = bot.id === selectedBotId;
              return (
                <button
                  key={bot.id}
                  type="button"
                  onClick={() => handleSelect(bot.id)}
                  className={cn(
                    "flex min-w-0 flex-1 flex-col items-center gap-1.5 rounded-2xl border px-2 py-2.5 transition-colors",
                    selected
                      ? "border-foreground/15 bg-white shadow-sm"
                      : "border-transparent bg-white/50 hover:bg-white",
                  )}
                >
                  <span
                    className={cn(
                      "flex size-11 items-center justify-center rounded-xl text-sm font-semibold",
                      getBotAvatarClass(bot.name),
                    )}
                  >
                    {getBotInitials(bot.name)}
                  </span>
                  <span className="w-full truncate text-center text-xs font-medium">
                    {bot.name}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      )}

      <Separator className="bg-border/50" />

      <ScrollArea className="min-h-0 flex-1 px-2 py-2">
        <Collapsible defaultOpen className="mb-1">
          <CollapsibleTrigger
            className="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-left text-[11px] font-semibold tracking-wide text-muted-foreground uppercase hover:bg-white/60"
          >
            Assistants
            <ChevronDown className="size-3.5 [[data-state=open]_&]:rotate-180" />
          </CollapsibleTrigger>
          <CollapsibleContent>
            <nav aria-label="Assistants" className="mt-0.5 space-y-0.5">
              {filteredBots.length === 0 ? (
                <p className="px-2 py-6 text-center text-xs text-muted-foreground">
                  {bots.length === 0
                    ? "No assistants yet. Create one to start."
                    : "No matches for your search."}
                </p>
              ) : (
                filteredBots.map((bot) => {
                  const selected = bot.id === selectedBotId;
                  return (
                    <button
                      key={bot.id}
                      type="button"
                      onClick={() => handleSelect(bot.id)}
                      className={cn(
                        "flex w-full items-start gap-2.5 rounded-xl px-2 py-2 text-left transition-colors",
                        selected ? "bg-white shadow-sm" : "hover:bg-white/70",
                      )}
                    >
                      <span
                        className={cn(
                          "mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg text-[11px] font-semibold",
                          getBotAvatarClass(bot.name),
                        )}
                      >
                        {getBotInitials(bot.name)}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="flex items-center gap-2">
                          <span className="truncate text-sm font-medium text-foreground">
                            {bot.name}
                          </span>
                          {selected && (
                            <span
                              className="size-2 shrink-0 rounded-full bg-sky-500"
                              aria-label="Selected"
                            />
                          )}
                        </span>
                        <span className="mt-0.5 line-clamp-1 text-xs text-muted-foreground">
                          {getBotPreview(bot)}
                        </span>
                      </span>
                      <span className="shrink-0 pt-0.5 text-[10px] text-muted-foreground">
                        {formatRelativeTime(bot.updatedAt)}
                      </span>
                    </button>
                  );
                })
              )}
            </nav>
          </CollapsibleContent>
        </Collapsible>

        <Collapsible defaultOpen={false} className="mb-1">
          <CollapsibleTrigger
            className="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-left text-[11px] font-semibold tracking-wide text-muted-foreground uppercase hover:bg-white/60"
          >
            Workspace
            <ChevronDown className="size-3.5 [[data-state=open]_&]:rotate-180" />
          </CollapsibleTrigger>
          <CollapsibleContent className="px-2 py-2 text-xs text-muted-foreground">
            Connect integrations to mirror channels here.
          </CollapsibleContent>
        </Collapsible>
      </ScrollArea>

      <div className="space-y-0.5 border-t border-border/50 p-2">
        <Button
          type="button"
          variant="ghost"
          className="h-9 w-full justify-start gap-2 rounded-xl text-sm font-normal text-muted-foreground"
          onClick={onCreateBot}
        >
          <Plus className="size-4" />
          New assistant
        </Button>
        <Button
          type="button"
          variant="ghost"
          className="h-9 w-full justify-start gap-2 rounded-xl text-sm font-normal text-muted-foreground"
          disabled
        >
          <Store className="size-4" />
          Marketplace
        </Button>
        <Button
          type="button"
          variant="ghost"
          className="h-9 w-full justify-start gap-2 rounded-xl text-sm font-normal text-muted-foreground"
          onClick={onOpenSettings}
        >
          <Settings2 className="size-4" />
          Settings
        </Button>
        <Button
          type="button"
          variant="ghost"
          className="h-9 w-full justify-start gap-2 rounded-xl text-sm font-normal text-muted-foreground"
          disabled
        >
          <Users className="size-4" />
          GPT Bot team
        </Button>
      </div>
    </div>
  );
}
