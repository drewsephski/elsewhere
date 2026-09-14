import type { Bot } from "@desktop/lib/definitions";
import { BotCreatureAvatar } from "@desktop/ui/bot-creature-avatar";
import { Button } from "@desktop/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@desktop/components/ui/sheet";
import { BotNav } from "@desktop/ui/bot-nav";
import { Menu, MoreHorizontal } from "@desktop/components/icons/lucide";

interface ChatHeaderProps {
  bot: Bot | null;
  mobileNavOpen: boolean;
  onMobileNavOpenChange: (open: boolean) => void;
  bots: Bot[];
  selectedBotId: string | null;
  onSelectBot: (id: string) => void;
  onCreateBot: () => void;
  onOpenSettings: () => void;
  onOpenVmDiagnostics?: () => void;
  isStreaming: boolean;
  apiKeyConfigured?: boolean;
  onRenameBot?: (id: string, name: string) => void | Promise<void>;
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
  apiKeyConfigured = false,
  onRenameBot,
}: ChatHeaderProps) {
  return (
    <header className="flex shrink-0 items-center gap-3 border-b border-border/60 bg-white px-4 py-3">
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        className="md:hidden"
        onClick={() => onMobileNavOpenChange(true)}
        aria-label="Open navigation"
      >
        <Menu className="size-4" />
      </Button>

      {bot ? (
        <div className="flex min-w-0 flex-1 items-center gap-2.5">
          <BotCreatureAvatar
            name={bot.name}
            size="md"
            animated={isStreaming}
          />
          <div className="min-w-0">
            <h1 className="truncate text-sm font-semibold tracking-tight">
              {bot.name}
            </h1>
            {isStreaming && (
              <p className="text-[11px] text-muted-foreground">Typing…</p>
            )}
          </div>
        </div>
      ) : (
        <h1 className="min-w-0 flex-1 text-sm font-semibold text-muted-foreground">
          Select an assistant
        </h1>
      )}

      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        className="text-muted-foreground"
        aria-label="More options"
        disabled
      >
        <MoreHorizontal className="size-4" />
      </Button>

      <Sheet open={mobileNavOpen} onOpenChange={onMobileNavOpenChange}>
        <SheetContent side="left" className="w-[min(100%,20rem)] p-0 sm:max-w-sm">
          <SheetHeader className="sr-only">
            <SheetTitle>Navigation</SheetTitle>
          </SheetHeader>
          <BotNav
            bots={bots}
            selectedBotId={selectedBotId}
            onSelectBot={onSelectBot}
            onCreateBot={onCreateBot}
            onOpenSettings={onOpenSettings}
            apiKeyConfigured={apiKeyConfigured}
            isStreaming={isStreaming}
            onRenameBot={onRenameBot}
            className="h-full"
            onNavigate={() => onMobileNavOpenChange(false)}
          />
        </SheetContent>
      </Sheet>
    </header>
  );
}
