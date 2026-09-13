import type { Bot } from "@/lib/definitions";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Bot as BotIcon, Plus, Settings2 } from "lucide-react";
import { cn } from "@/lib/utils";

interface BotNavProps {
  bots: Bot[];
  selectedBotId: string | null;
  onSelectBot: (id: string) => void;
  onCreateBot: () => void;
  onOpenSettings: () => void;
  className?: string;
  onNavigate?: () => void;
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
  function handleSelect(id: string) {
    onSelectBot(id);
    onNavigate?.();
  }

  return (
    <div className={cn("flex h-full flex-col", className)}>
      <div className="flex items-center justify-between gap-2 px-3 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <div
            className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/15 text-primary"
            aria-hidden
          >
            <BotIcon className="size-4" />
          </div>
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold tracking-tight">GPT Bot</p>
            <p className="truncate text-[11px] text-muted-foreground">Local assistants</p>
          </div>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          onClick={onOpenSettings}
          aria-label="Open settings"
        >
          <Settings2 className="size-4" />
        </Button>
      </div>

      <Separator />

      <div className="p-2">
        <Button
          type="button"
          variant="outline"
          className="w-full justify-start gap-2 border-dashed"
          onClick={onCreateBot}
        >
          <Plus className="size-4" />
          New bot
        </Button>
      </div>

      <ScrollArea className="flex-1 px-2 pb-3">
        <nav aria-label="Bots">
          {bots.length === 0 ? (
            <p className="px-2 py-6 text-center text-xs text-muted-foreground">
              No bots yet. Create one to start chatting.
            </p>
          ) : (
            <ul className="space-y-0.5">
              {bots.map((bot) => {
                const selected = bot.id === selectedBotId;
                return (
                  <li key={bot.id}>
                    <button
                      type="button"
                      onClick={() => handleSelect(bot.id)}
                      className={cn(
                        "flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm transition-colors",
                        selected
                          ? "bg-sidebar-accent text-sidebar-accent-foreground"
                          : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
                      )}
                      title={bot.name}
                    >
                      <span
                        className={cn(
                          "size-1.5 shrink-0 rounded-full",
                          selected ? "bg-primary" : "bg-muted-foreground/40",
                        )}
                        aria-hidden
                      />
                      <span className="truncate font-medium">{bot.name}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </nav>
      </ScrollArea>
    </div>
  );
}
