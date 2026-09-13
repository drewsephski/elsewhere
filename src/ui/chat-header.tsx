import type { Bot } from "@/lib/definitions";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { BotNav } from "@/ui/bot-nav";
import { Menu, Sparkles } from "lucide-react";

interface ChatHeaderProps {
  bot: Bot | null;
  mobileNavOpen: boolean;
  onMobileNavOpenChange: (open: boolean) => void;
  bots: Bot[];
  selectedBotId: string | null;
  onSelectBot: (id: string) => void;
  onCreateBot: () => void;
  onOpenSettings: () => void;
  isStreaming: boolean;
}

export function ChatHeader({
  bot,
  mobileNavOpen,
  onMobileNavOpenChange,
  bots,
  selectedBotId,
  onSelectBot,
  onCreateBot,
  onOpenSettings,
  isStreaming,
}: ChatHeaderProps) {
  return (
    <header className="flex shrink-0 items-center gap-2 border-b border-border/80 bg-background/80 px-3 py-2.5 backdrop-blur-md sm:px-4">
      <Button
        type="button"
        variant="outline"
        size="icon-sm"
        className="md:hidden"
        onClick={() => onMobileNavOpenChange(true)}
        aria-label="Open bot list"
      >
        <Menu className="size-4" />
      </Button>

      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <h1 className="truncate text-sm font-semibold tracking-tight sm:text-base">
            {bot?.name ?? "Select a bot"}
          </h1>
          {bot && (
            <Badge variant="secondary" className="hidden font-mono text-[10px] sm:inline-flex">
              {bot.model}
            </Badge>
          )}
          {isStreaming && (
            <Badge className="gap-1 border-primary/30 bg-primary/10 text-primary">
              <Sparkles className="size-3" aria-hidden />
              Responding
            </Badge>
          )}
        </div>
        {bot?.systemPrompt ? (
          <p className="mt-0.5 line-clamp-1 text-xs text-muted-foreground">
            {bot.systemPrompt}
          </p>
        ) : (
          <p className="mt-0.5 text-xs text-muted-foreground">
            {bot ? "No system instructions" : "Choose or create a bot to begin"}
          </p>
        )}
      </div>

      <Sheet open={mobileNavOpen} onOpenChange={onMobileNavOpenChange}>
        <SheetContent side="left" className="w-[min(100%,18rem)] p-0 sm:max-w-xs">
          <SheetHeader className="sr-only">
            <SheetTitle>Bots</SheetTitle>
          </SheetHeader>
          <BotNav
            bots={bots}
            selectedBotId={selectedBotId}
            onSelectBot={onSelectBot}
            onCreateBot={onCreateBot}
            onOpenSettings={onOpenSettings}
            className="h-full"
            onNavigate={() => onMobileNavOpenChange(false)}
          />
        </SheetContent>
      </Sheet>
    </header>
  );
}
