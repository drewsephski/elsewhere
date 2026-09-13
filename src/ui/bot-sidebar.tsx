import type { Bot } from "@/lib/definitions";

interface BotSidebarProps {
  bots: Bot[];
  selectedBotId: string | null;
  onSelectBot: (id: string) => void;
  onCreateBot: () => void;
  onOpenSettings: () => void;
}

export function BotSidebar({
  bots,
  selectedBotId,
  onSelectBot,
  onCreateBot,
  onOpenSettings,
}: BotSidebarProps) {
  return (
    <aside className="w-56 shrink-0 flex flex-col border-r border-border bg-surface-1">
      <div className="px-3 py-3 border-b border-border flex items-center justify-between">
        <span className="font-semibold text-sm tracking-tight">GPT Bot</span>
        <button
          type="button"
          onClick={onOpenSettings}
          className="text-xs text-muted hover:text-foreground px-2 py-1 rounded hover:bg-surface-2"
          aria-label="Settings"
        >
          Settings
        </button>
      </div>
      <div className="px-2 py-2">
        <button
          type="button"
          onClick={onCreateBot}
          className="w-full text-left text-sm rounded-md border border-dashed border-border px-3 py-2 text-muted hover:text-foreground hover:border-muted hover:bg-surface-2"
        >
          + New Bot
        </button>
      </div>
      <nav className="flex-1 overflow-y-auto px-2 pb-2" aria-label="Bots">
        {bots.length === 0 ? (
          <p className="text-xs text-muted px-2 py-4">No bots yet.</p>
        ) : (
          <ul className="space-y-0.5">
            {bots.map((bot) => {
              const selected = bot.id === selectedBotId;
              return (
                <li key={bot.id}>
                  <button
                    type="button"
                    onClick={() => onSelectBot(bot.id)}
                    className={`w-full text-left rounded-md px-3 py-2 text-sm truncate ${
                      selected
                        ? "bg-surface-3 text-foreground"
                        : "text-muted hover:bg-surface-2 hover:text-foreground"
                    }`}
                    title={bot.name}
                  >
                    {bot.name}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </nav>
    </aside>
  );
}
