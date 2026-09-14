"use client";

import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { InlineRenameLabel } from "@/components/app/inline-rename-label";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { formatMessageTime } from "@/lib/format";
import {
  activityPreview,
  presenceLabels,
  presenceIsActive,
  presenceNeedsAttention,
  type WorkspaceBotPresence,
} from "@/lib/workspace-types";
import { cn } from "cn";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Plus, Search } from "@/components/icons/lucide";
import type { GroupListItem } from "@/lib/api-types";
import type { WorkspaceLoadPhase } from "@/hooks/use-workspace-overview";
import { workspaceBotsEmptyMessage } from "@/lib/workspace-load-state";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

interface BotListSidebarProps {
  bots: WorkspaceBotPresence[];
  groups?: GroupListItem[];
  selectedBotId: string | null;
  selectedGroupId?: string | null;
  runActivityAt: Record<string, string>;
  workspacePhase?: WorkspaceLoadPhase;
  workspaceError?: string | null;
  onCreateBot: () => void;
  onCreateGroup?: () => void;
  onRenameBot?: (botId: string, name: string) => Promise<void>;
  onDeleteBot?: (botId: string) => Promise<void>;
  footer: React.ReactNode;
  className?: string;
}

interface ContextMenuState {
  bot: WorkspaceBotPresence;
  x: number;
  y: number;
}

const menuItemClass =
  "flex w-full cursor-default items-center rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground focus-visible:bg-accent";

export function BotListSidebar({
  bots,
  groups = [],
  selectedBotId,
  selectedGroupId = null,
  runActivityAt,
  workspacePhase = "ready",
  workspaceError = null,
  onCreateBot,
  onCreateGroup,
  onRenameBot,
  onDeleteBot,
  footer,
  className,
}: BotListSidebarProps) {
  const [query, setQuery] = useState("");
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [renameBot, setRenameBot] = useState<WorkspaceBotPresence | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [deleteBot, setDeleteBot] = useState<WorkspaceBotPresence | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) {
      return bots;
    }
    return bots.filter((bot) => bot.name.toLowerCase().includes(normalized));
  }, [bots, query]);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }
    function handleDismiss() {
      closeContextMenu();
    }
    function handleKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        closeContextMenu();
      }
    }
    window.addEventListener("click", handleDismiss);
    window.addEventListener("scroll", handleDismiss, true);
    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("click", handleDismiss);
      window.removeEventListener("scroll", handleDismiss, true);
      window.removeEventListener("keydown", handleKey);
    };
  }, [closeContextMenu, contextMenu]);

  function handleContextMenu(event: React.MouseEvent, bot: WorkspaceBotPresence) {
    if (!onRenameBot && !onDeleteBot) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    setContextMenu({ bot, x: event.clientX, y: event.clientY });
  }

  function openRenameDialog(bot: WorkspaceBotPresence) {
    setRenameBot(bot);
    setRenameDraft(bot.name);
    setActionError(null);
  }

  async function handleRenameSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!renameBot || !onRenameBot || actionBusy) {
      return;
    }
    const trimmed = renameDraft.trim();
    if (!trimmed) {
      return;
    }
    setActionBusy(true);
    setActionError(null);
    try {
      await onRenameBot(renameBot.id, trimmed);
      setRenameBot(null);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Could not rename bot");
    } finally {
      setActionBusy(false);
    }
  }

  async function handleDeleteConfirm() {
    if (!deleteBot || !onDeleteBot || actionBusy) {
      return;
    }
    setActionBusy(true);
    setActionError(null);
    try {
      await onDeleteBot(deleteBot.id);
      setDeleteBot(null);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Could not delete bot");
    } finally {
      setActionBusy(false);
    }
  }

  return (
    <div className={cn("flex h-full min-h-0 flex-col", className)}>
      <div className="shrink-0 space-y-2.5 px-2.5 pt-2.5 sm:px-3 sm:pt-3 sm:space-y-3">
        <div className="relative">
          <Search
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <Input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search"
            className="h-9 rounded-xl border-border/80 bg-white/80 py-2 pr-2.5 pl-9 text-sm sm:h-10 sm:pr-3"
            aria-label="Search bots"
          />
        </div>
        <Button
          type="button"
          variant="outline"
          onClick={onCreateBot}
          className="w-full gap-2 rounded-xl border-dashed border-primary/35 bg-primary/5 text-primary hover:bg-primary/10"
        >
          <Plus className="size-4" aria-hidden />
          New bot
        </Button>
        {onCreateGroup ? (
          <Button
            type="button"
            variant="outline"
            onClick={onCreateGroup}
            className="w-full gap-2 rounded-xl border-border/80 bg-white/80"
          >
            New group
          </Button>
        ) : null}
        {bots.length > 0 ? (
          <div className="flex gap-3 overflow-x-auto pb-1 lg:hidden" aria-label="Quick access">
            {bots.slice(0, 5).map((bot) => (
              <Link
                key={bot.id}
                href={`/app/bots/${bot.id}`}
                className="flex w-14 shrink-0 flex-col items-center gap-0.5"
                onContextMenu={(event) => handleContextMenu(event, bot)}
              >
                <BotCreatureAvatar
                  name={bot.name}
                  avatarId={bot.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                  size="lg"
                />
                <span className="w-full truncate text-center text-[10px] font-medium">
                  {bot.name.split(" ")[0]}
                </span>
              </Link>
            ))}
          </div>
        ) : null}
      </div>

      <div className="mt-1.5 min-h-0 flex-1 space-y-3 overflow-y-auto px-1.5 pb-2 sm:mt-2 sm:px-2">
        {groups.length > 0 ? (
          <section aria-label="Groups">
            <p className="px-2 pb-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Groups
            </p>
            <ul className="space-y-0.5">
              {groups.map((group) => {
                const selected = group.id === selectedGroupId;
                const active = group.workingRuns > 0 || group.queuedRuns > 0;
                return (
                  <li key={group.id}>
                    <Link
                      href={`/app/groups/${group.id}`}
                      className={cn(
                        "flex items-center gap-2 rounded-xl border px-2 py-1.5 transition-colors",
                        selected
                          ? "border-primary/12 bg-white/95 shadow-sm"
                          : "border-transparent hover:border-border/60 hover:bg-white/80",
                      )}
                      aria-current={selected ? "page" : undefined}
                    >
                      <div className="flex -space-x-1.5">
                        {group.participants
                          .filter((p) => !p.leftAt)
                          .slice(0, 3)
                          .map((participant) => (
                            <BotCreatureAvatar
                              key={participant.botId}
                              name={participant.name}
                              avatarId={participant.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                              size="sm"
                              className="ring-1 ring-white"
                            />
                          ))}
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1">
                          <span className="truncate text-[13px] font-semibold">{group.name}</span>
                          <span className="ml-auto shrink-0 text-[10px] text-muted-foreground">
                            {formatMessageTime(group.updatedAt)}
                          </span>
                        </div>
                        <p className="text-[11px] text-muted-foreground">
                          {active
                            ? group.workingRuns > 0
                              ? "Working…"
                              : "Queued…"
                            : `${group.participants.filter((p) => !p.leftAt).length} participants`}
                        </p>
                      </div>
                    </Link>
                  </li>
                );
              })}
            </ul>
          </section>
        ) : null}
        <section aria-label="Bots">
          <p className="px-2 pb-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
            Bots
          </p>
          <ul className="space-y-0.5" aria-label="Bots">
        {filtered.map((bot) => {
          const selected = bot.id === selectedBotId;
          const attention = presenceNeedsAttention(bot.presence);
          const showAttention = attention && !selected;
          const activityIso = runActivityAt[bot.id];
          const timeLabel = activityIso ? formatMessageTime(activityIso) : null;

          return (
            <li key={bot.id}>
              <Link
                href={`/app/bots/${bot.id}`}
                onContextMenu={(event) => handleContextMenu(event, bot)}
                className={cn(
                  "group flex items-center gap-2 rounded-xl border px-2 py-1.5 transition-colors duration-150",
                  selected
                    ? "border-primary/12 bg-white/95 shadow-sm shadow-primary/[0.04]"
                    : "border-transparent hover:border-border/60 hover:bg-white/80",
                )}
                aria-current={selected ? "page" : undefined}
              >
                <BotCreatureAvatar
                  name={bot.name}
                  avatarId={bot.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                  size="md"
                  animated={selected && presenceIsActive(bot.presence)}
                />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-1.5">
                    {onRenameBot ? (
                      <InlineRenameLabel
                        value={bot.name}
                        onCommit={(next) => onRenameBot(bot.id, next)}
                        className="text-[13px] font-semibold leading-tight"
                        inputClassName="text-[13px]"
                        ariaLabel={`Rename ${bot.name}`}
                      />
                    ) : (
                      <span className="truncate text-[13px] font-semibold leading-tight">
                        {bot.name}
                      </span>
                    )}
                    {timeLabel ? (
                      <span className="ml-auto shrink-0 text-[10px] text-muted-foreground">
                        {timeLabel}
                      </span>
                    ) : null}
                  </div>
                  <p className="mt-0.5 line-clamp-1 text-[11px] leading-snug text-muted-foreground">
                    {showAttention
                      ? (presenceLabels[bot.presence] ?? "Needs attention")
                      : activityPreview(bot)}
                  </p>
                </div>
                {showAttention ? (
                  <span
                    className="size-1.5 shrink-0 rounded-full bg-primary"
                    aria-label="Needs attention"
                  />
                ) : null}
              </Link>
            </li>
          );
        })}
        {!filtered.length ? (
          <li className="px-3 py-8 text-center text-sm text-muted-foreground">
            {workspaceBotsEmptyMessage(workspacePhase, {
              query,
              workspaceError,
            })}
          </li>
        ) : null}
          </ul>
        </section>
      </div>

      <div className="shrink-0 border-t border-border/70 bg-white/50 px-2.5 py-2.5 sm:px-3 sm:py-3">{footer}</div>

      {contextMenu ? (
        <div
          className="fixed z-[100] min-w-40 rounded-lg border border-border/80 bg-popover p-1 text-popover-foreground shadow-lg ring-1 ring-foreground/10"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          role="menu"
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          {onRenameBot ? (
            <button
              type="button"
              role="menuitem"
              className={menuItemClass}
              onClick={() => {
                openRenameDialog(contextMenu.bot);
                closeContextMenu();
              }}
            >
              Rename
            </button>
          ) : null}
          {onDeleteBot ? (
            <button
              type="button"
              role="menuitem"
              className={cn(menuItemClass, "text-red-700 hover:bg-red-50 hover:text-red-800")}
              onClick={() => {
                setDeleteBot(contextMenu.bot);
                setActionError(null);
                closeContextMenu();
              }}
            >
              Delete
            </button>
          ) : null}
        </div>
      ) : null}

      <Dialog open={Boolean(renameBot)} onOpenChange={(open) => !open && setRenameBot(null)}>
        <DialogContent className="sm:max-w-sm">
          <form onSubmit={(event) => void handleRenameSubmit(event)}>
            <DialogHeader>
              <DialogTitle>Rename bot</DialogTitle>
              <DialogDescription>Choose a name your team will recognize.</DialogDescription>
            </DialogHeader>
            <div className="mt-4 space-y-2">
              <Label htmlFor="sidebar-rename-bot">Name</Label>
              <Input
                id="sidebar-rename-bot"
                value={renameDraft}
                onChange={(event) => setRenameDraft(event.target.value)}
                maxLength={100}
                required
                autoFocus
              />
            </div>
            {actionError ? (
              <p className="mt-2 text-sm text-red-700" role="alert">{actionError}</p>
            ) : null}
            <DialogFooter className="mt-4">
              <Button type="button" variant="outline" onClick={() => setRenameBot(null)}>
                Cancel
              </Button>
              <Button type="submit" disabled={actionBusy || !renameDraft.trim()}>
                {actionBusy ? "Saving…" : "Save"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={Boolean(deleteBot)} onOpenChange={(open) => !open && setDeleteBot(null)}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Delete {deleteBot?.name}?</DialogTitle>
            <DialogDescription>
              This removes the bot and its settings. Work history may remain in your account.
            </DialogDescription>
          </DialogHeader>
          {actionError ? (
            <p className="text-sm text-red-700" role="alert">{actionError}</p>
          ) : null}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setDeleteBot(null)}>
              Cancel
            </Button>
            <Button
              type="button"
              className="bg-red-700 text-white hover:bg-red-800"
              disabled={actionBusy}
              onClick={() => void handleDeleteConfirm()}
            >
              {actionBusy ? "Deleting…" : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
