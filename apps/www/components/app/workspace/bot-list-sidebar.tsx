"use client";

import { botAvatarClass, getBotInitials } from "@/lib/bot-visual";
import { formatMessageTime } from "@/lib/format";
import {
  activityPreview,
  presenceLabels,
  presenceNeedsAttention,
  type WorkspaceBotPresence,
} from "@/lib/workspace-types";
import { cn } from "cn";
import { Plus, Search } from "lucide-react";
import Link from "next/link";
import { useMemo, useState } from "react";

interface BotListSidebarProps {
  bots: WorkspaceBotPresence[];
  selectedBotId: string | null;
  runActivityAt: Record<string, string>;
  onCreateBot: () => void;
  footer: React.ReactNode;
  className?: string;
}

export function BotListSidebar({
  bots,
  selectedBotId,
  runActivityAt,
  onCreateBot,
  footer,
  className,
}: BotListSidebarProps) {
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) {
      return bots;
    }
    return bots.filter((bot) => bot.name.toLowerCase().includes(normalized));
  }, [bots, query]);

  return (
    <div className={cn("flex h-full min-h-0 flex-col", className)}>
      <div className="shrink-0 space-y-3 px-3 pt-3">
        <div className="relative">
          <Search
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search"
            className="h-10 w-full rounded-xl border border-border/80 bg-white/80 py-2 pr-3 pl-9 text-sm outline-none ring-primary/30 placeholder:text-muted-foreground focus:ring-2"
            aria-label="Search bots"
          />
        </div>
        <button
          type="button"
          onClick={onCreateBot}
          className="flex w-full items-center justify-center gap-2 rounded-xl border border-dashed border-primary/35 bg-primary/5 px-3 py-2.5 text-sm font-medium text-primary transition-colors hover:bg-primary/10"
        >
          <Plus className="size-4" aria-hidden />
          New bot
        </button>
        {bots.length > 0 ? (
          <div className="flex gap-3 overflow-x-auto pb-1 lg:hidden" aria-label="Quick access">
            {bots.slice(0, 5).map((bot) => (
              <Link
                key={bot.id}
                href={`/app/bots/${bot.id}`}
                className="flex w-16 shrink-0 flex-col items-center gap-1"
              >
                <span
                  className={cn(
                    "flex size-12 items-center justify-center rounded-2xl text-xs font-semibold ring-1",
                    botAvatarClass(bot.id),
                  )}
                >
                  {getBotInitials(bot.name)}
                </span>
                <span className="w-full truncate text-center text-[10px] font-medium">
                  {bot.name.split(" ")[0]}
                </span>
              </Link>
            ))}
          </div>
        ) : null}
      </div>

      <ul className="mt-2 min-h-0 flex-1 space-y-0.5 overflow-y-auto px-2 pb-2" aria-label="Bots">
        {filtered.map((bot) => {
          const selected = bot.id === selectedBotId;
          const attention = presenceNeedsAttention(bot.presence);
          const activityIso = runActivityAt[bot.id];
          const timeLabel = activityIso ? formatMessageTime(activityIso) : null;

          return (
            <li key={bot.id}>
              <Link
                href={`/app/bots/${bot.id}`}
                className={cn(
                  "group flex items-start gap-3 rounded-xl px-2.5 py-2.5 transition-colors",
                  selected
                    ? "bg-primary/10 ring-1 ring-primary/15"
                    : "hover:bg-white/70",
                )}
                aria-current={selected ? "page" : undefined}
              >
                <span
                  className={cn(
                    "flex size-11 shrink-0 items-center justify-center rounded-2xl text-sm font-semibold ring-1",
                    botAvatarClass(bot.id),
                  )}
                  aria-hidden
                >
                  {getBotInitials(bot.name)}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-semibold">{bot.name}</span>
                    {timeLabel ? (
                      <span className="ml-auto shrink-0 text-[11px] text-muted-foreground">
                        {timeLabel}
                      </span>
                    ) : null}
                  </div>
                  <p className="mt-0.5 line-clamp-2 text-xs leading-relaxed text-muted-foreground">
                    {activityPreview(bot)}
                  </p>
                  {attention ? (
                    <p className="mt-1 text-[11px] font-medium text-amber-800">
                      {presenceLabels[bot.presence] ?? "Needs attention"}
                    </p>
                  ) : null}
                </div>
                {attention ? (
                  <span
                    className="mt-2 size-2 shrink-0 rounded-full bg-primary"
                    aria-label="Needs attention"
                  />
                ) : null}
              </Link>
            </li>
          );
        })}
        {!filtered.length ? (
          <li className="px-3 py-8 text-center text-sm text-muted-foreground">
            {query ? "No bots match your search." : "No bots yet."}
          </li>
        ) : null}
      </ul>

      <div className="shrink-0 border-t border-border/70 bg-white/50 px-3 py-3">{footer}</div>
    </div>
  );
}
