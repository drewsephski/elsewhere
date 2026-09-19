"use client";

import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { InlineRenameLabel } from "@/components/app/inline-rename-label";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { BOT_DELETE_COPY } from "@/lib/bot-delete-copy";
import { formatMessageTime } from "@/lib/format";
import {
  activityPreview,
  presenceDotClass,
  presenceIsActive,
  presenceSidebarTrailing,
  type WorkspaceBotPresence,
} from "@/lib/workspace-types";
import { cn } from "cn";
import { LayoutGroup, motion, useReducedMotion } from "motion/react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Plus, Search } from "@/components/icons/lucide";
import { ProductLogo } from "@/components/product-logo";
import { ComposerIconButton } from "./chat-composer";
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
  onRenameGroup?: (groupId: string, name: string) => Promise<void>;
  onDeleteGroup?: (groupId: string) => Promise<void>;
  footer: React.ReactNode;
  className?: string;
}

type SidebarItemKind = "bot" | "group";

interface SidebarNamedItem {
  kind: SidebarItemKind;
  id: string;
  name: string;
}

interface ContextMenuState extends SidebarNamedItem {
  x: number;
  y: number;
}

const menuItemClass =
  "flex w-full cursor-default items-center rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground focus-visible:bg-accent";

const navRowClass =
  "group relative flex items-center gap-2.5 rounded-lg px-2 py-1.5 transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50";
const navRowIdleClass = "hover:bg-surface-hover";
const BOT_LIST_SELECTION_LAYOUT_ID = "bot-list-selection";
const selectionSpring = {
  type: "spring" as const,
  stiffness: 420,
  damping: 34,
  mass: 0.75,
};

function SidebarNavLink({
  href,
  selected,
  reduceMotion,
  onContextMenu,
  children,
}: {
  href: string;
  selected: boolean;
  reduceMotion: boolean | null;
  onContextMenu: (event: React.MouseEvent) => void;
  children: React.ReactNode;
}) {
  return (
    <Link
      href={href}
      onContextMenu={onContextMenu}
      className={cn(navRowClass, !selected && navRowIdleClass)}
      aria-current={selected ? "page" : undefined}
    >
      {selected ? (
        <motion.span
          layoutId={BOT_LIST_SELECTION_LAYOUT_ID}
          data-sidebar-selection=""
          className="pointer-events-none absolute inset-0 rounded-lg bg-surface-active"
          transition={reduceMotion ? { duration: 0 } : selectionSpring}
        />
      ) : null}
      <span className="relative z-10 flex min-w-0 flex-1 items-center gap-2.5">
        {children}
      </span>
    </Link>
  );
}

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
  onRenameGroup,
  onDeleteGroup,
  footer,
  className,
}: BotListSidebarProps) {
  const [query, setQuery] = useState("");
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [renameItem, setRenameItem] = useState<SidebarNamedItem | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [deleteItem, setDeleteItem] = useState<SidebarNamedItem | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const reduceMotion = useReducedMotion();

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

  function canRename(kind: SidebarItemKind): boolean {
    return kind === "bot" ? Boolean(onRenameBot) : Boolean(onRenameGroup);
  }

  function canDelete(kind: SidebarItemKind): boolean {
    return kind === "bot" ? Boolean(onDeleteBot) : Boolean(onDeleteGroup);
  }

  function handleContextMenu(event: React.MouseEvent, item: SidebarNamedItem) {
    if (!canRename(item.kind) && !canDelete(item.kind)) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    setContextMenu({ ...item, x: event.clientX, y: event.clientY });
  }

  function openRenameDialog(item: SidebarNamedItem) {
    setRenameItem(item);
    setRenameDraft(item.name);
    setActionError(null);
  }

  async function handleRenameSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!renameItem || actionBusy) {
      return;
    }
    const trimmed = renameDraft.trim();
    if (!trimmed) {
      return;
    }
    const rename =
      renameItem.kind === "bot" ? onRenameBot : onRenameGroup;
    if (!rename) {
      return;
    }
    setActionBusy(true);
    setActionError(null);
    try {
      await rename(renameItem.id, trimmed);
      setRenameItem(null);
    } catch (err) {
      setActionError(
        err instanceof Error
          ? err.message
          : renameItem.kind === "bot"
            ? "Could not rename bot"
            : "Could not rename group",
      );
    } finally {
      setActionBusy(false);
    }
  }

  async function handleDeleteConfirm() {
    if (!deleteItem || actionBusy) {
      return;
    }
    const remove = deleteItem.kind === "bot" ? onDeleteBot : onDeleteGroup;
    if (!remove) {
      return;
    }
    setActionBusy(true);
    setActionError(null);
    try {
      await remove(deleteItem.id);
      setDeleteItem(null);
    } catch (err) {
      setActionError(
        err instanceof Error
          ? err.message
          : deleteItem.kind === "bot"
            ? "Could not delete bot"
            : "Could not delete group",
      );
    } finally {
      setActionBusy(false);
    }
  }

  return (
    <div className={cn("flex h-full min-h-0 flex-col", className)}>
      <div className="flex h-11 shrink-0 items-center justify-between gap-2 px-3 tauri-traffic-safe-l">
        <Link
          href="/app"
          className="flex items-center gap-2 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          aria-label="Workspace home"
        >
          <ProductLogo size="md" className="opacity-90" />
        </Link>
        {onCreateGroup ? (
          <DropdownMenu>
            <DropdownMenuTrigger
              render={<ComposerIconButton label="New…" className="size-7" />}
            >
              <Plus className="size-4" aria-hidden />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-40">
              <DropdownMenuItem onClick={onCreateBot}>New bot</DropdownMenuItem>
              <DropdownMenuItem onClick={onCreateGroup}>New group</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        ) : (
          <ComposerIconButton label="New bot" className="size-7" onClick={onCreateBot}>
            <Plus className="size-4" aria-hidden />
          </ComposerIconButton>
        )}
      </div>

      <div className="shrink-0 space-y-2 px-2.5 pb-1">
        <div className="relative">
          <Search
            className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <Input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search"
            className="h-8 rounded-lg border-transparent bg-surface-raised pr-2.5 pl-8 text-[13px] placeholder:text-muted-foreground focus-visible:border-ring/40 focus-visible:ring-0 dark:bg-surface-raised"
            aria-label="Search bots"
          />
        </div>
        {bots.length > 0 ? (
          <div className="flex gap-3 overflow-x-auto pb-1 lg:hidden" aria-label="Quick access">
            {bots.slice(0, 5).map((bot) => (
              <Link
                key={bot.id}
                href={`/app/bots/${bot.id}`}
                className="flex w-14 shrink-0 flex-col items-center gap-1"
                onContextMenu={(event) =>
                  handleContextMenu(event, { kind: "bot", id: bot.id, name: bot.name })
                }
              >
                <BotCreatureAvatar
                  name={bot.name}
                  avatarId={bot.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                  size="lg"
                  animated={presenceIsActive(bot.presence)}
                />
                <span className="w-full truncate text-center text-[10px] font-medium" title={bot.name}>
                  {bot.name}
                </span>
              </Link>
            ))}
          </div>
        ) : null}
      </div>

      <LayoutGroup id="bot-list-sidebar">
        <div className="mt-1 min-h-0 flex-1 space-y-2 overflow-y-auto px-2 pb-2">
          {groups.length > 0 ? (
            <section aria-label="Groups">
              <p className="px-2 pb-1 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground/80">
                Groups
              </p>
              <ul className="space-y-px">
                {groups.map((group) => {
                  const selected = group.id === selectedGroupId;
                  const active = group.workingRuns > 0 || group.queuedRuns > 0;
                  const participants = group.participants.filter((p) => !p.leftAt);
                  return (
                    <li key={group.id}>
                      <SidebarNavLink
                        href={`/app/groups/${group.id}`}
                        selected={selected}
                        reduceMotion={reduceMotion}
                        onContextMenu={(event) =>
                          handleContextMenu(event, {
                            kind: "group",
                            id: group.id,
                            name: group.name,
                          })
                        }
                      >
                        <div className="flex shrink-0 -space-x-2">
                          {participants.slice(0, 3).map((participant) => (
                            <BotCreatureAvatar
                              key={participant.botId}
                              name={participant.name}
                              avatarId={participant.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                              size="xs"
                              variant="tile"
                              className="ring-2 ring-surface"
                            />
                          ))}
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-1.5">
                            {onRenameGroup ? (
                              <InlineRenameLabel
                                value={group.name}
                                nested
                                onCommit={(next) => onRenameGroup(group.id, next)}
                                className="text-[13px] font-medium leading-tight"
                                inputClassName="text-[13px]"
                                ariaLabel={`Rename ${group.name}`}
                              />
                            ) : (
                              <span className="truncate text-[13px] font-medium leading-tight">
                                {group.name}
                              </span>
                            )}
                            <span className="ml-auto shrink-0 text-[10px] text-muted-foreground/80">
                              {formatMessageTime(group.updatedAt)}
                            </span>
                          </div>
                          <p className="mt-0.5 line-clamp-1 text-[11px] leading-snug text-muted-foreground">
                            {active
                              ? group.workingRuns > 0
                                ? "Working…"
                                : "Queued…"
                              : `${participants.length} participants`}
                          </p>
                        </div>
                      </SidebarNavLink>
                    </li>
                  );
                })}
              </ul>
            </section>
          ) : null}
          <section aria-label="Bots" className={cn(groups.length > 0 && "pt-2")}>
            {groups.length > 0 ? (
              <p className="px-2 pb-1 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground/80">
                Bots
              </p>
            ) : null}
            <ul className="space-y-px">
              {filtered.map((bot) => {
                const selected = bot.id === selectedBotId;
                const { statusLabel, emphasis } = presenceSidebarTrailing(bot.presence);
                const activityIso = runActivityAt[bot.id];
                const timeLabel = activityIso ? formatMessageTime(activityIso) : null;
                const trailingLabel = statusLabel ?? timeLabel;

                return (
                  <li key={bot.id}>
                    <SidebarNavLink
                      href={`/app/bots/${bot.id}`}
                      selected={selected}
                      reduceMotion={reduceMotion}
                      onContextMenu={(event) =>
                        handleContextMenu(event, { kind: "bot", id: bot.id, name: bot.name })
                      }
                    >
                      <BotCreatureAvatar
                        name={bot.name}
                        avatarId={bot.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                        size="sm"
                        animated={presenceIsActive(bot.presence)}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5">
                          {onRenameBot ? (
                            <InlineRenameLabel
                              value={bot.name}
                              nested
                              onCommit={(next) => onRenameBot(bot.id, next)}
                              className="text-[13px] font-medium leading-tight"
                              inputClassName="text-[13px]"
                              ariaLabel={`Rename ${bot.name}`}
                            />
                          ) : (
                            <span className="truncate text-[13px] font-medium leading-tight">
                              {bot.name}
                            </span>
                          )}
                          {trailingLabel ? (
                            <span
                              className={cn(
                                "ml-auto inline-flex shrink-0 items-center gap-1 text-[10px]",
                                emphasis === "attention"
                                  ? "rounded-full bg-warning/15 px-1.5 py-0.5 font-medium text-warning"
                                  : emphasis === "active"
                                    ? "font-medium text-info"
                                    : "text-muted-foreground/80",
                              )}
                            >
                              {statusLabel ? (
                                <span
                                  className={cn("size-1.5 rounded-full", presenceDotClass(bot.presence))}
                                  aria-hidden
                                />
                              ) : null}
                              {trailingLabel}
                            </span>
                          ) : null}
                        </div>
                        <p className="mt-0.5 line-clamp-1 text-[11px] leading-snug text-muted-foreground">
                          {activityPreview(bot)}
                        </p>
                      </div>
                    </SidebarNavLink>
                  </li>
                );
              })}
              {!filtered.length ? (
                <li className="px-3 py-8 text-center text-[13px] text-muted-foreground">
                  {workspaceBotsEmptyMessage(workspacePhase, {
                    query,
                    workspaceError,
                  })}
                </li>
              ) : null}
            </ul>
          </section>
        </div>
      </LayoutGroup>

      <div className="shrink-0 px-2 pb-2 pt-1">{footer}</div>

      {contextMenu ? (
        <div
          className="fixed z-[100] min-w-40 rounded-lg border border-border bg-popover p-1 text-popover-foreground shadow-xl"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          role="menu"
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          {canRename(contextMenu.kind) ? (
            <button
              type="button"
              role="menuitem"
              className={menuItemClass}
              onClick={() => {
                openRenameDialog(contextMenu);
                closeContextMenu();
              }}
            >
              Rename
            </button>
          ) : null}
          {canDelete(contextMenu.kind) ? (
            <button
              type="button"
              role="menuitem"
              className={cn(menuItemClass, "text-destructive hover:bg-destructive/10 hover:text-destructive")}
              onClick={() => {
                setDeleteItem(contextMenu);
                setActionError(null);
                closeContextMenu();
              }}
            >
              Delete
            </button>
          ) : null}
        </div>
      ) : null}

      <Dialog open={Boolean(renameItem)} onOpenChange={(open) => !open && setRenameItem(null)}>
        <DialogContent className="sm:max-w-sm">
          <form onSubmit={(event) => void handleRenameSubmit(event)}>
            <DialogHeader>
              <DialogTitle>
                {renameItem?.kind === "group" ? "Rename group" : "Rename bot"}
              </DialogTitle>
              <DialogDescription>
                {renameItem?.kind === "group"
                  ? "Choose a name for this group conversation."
                  : "Choose a name your team will recognize."}
              </DialogDescription>
            </DialogHeader>
            <div className="mt-4 space-y-2">
              <Label htmlFor="sidebar-rename-item">Name</Label>
              <Input
                id="sidebar-rename-item"
                value={renameDraft}
                onChange={(event) => setRenameDraft(event.target.value)}
                maxLength={renameItem?.kind === "group" ? 200 : 100}
                required
                autoFocus
              />
            </div>
            {actionError ? (
              <p className="mt-2 text-sm text-destructive" role="alert">{actionError}</p>
            ) : null}
            <DialogFooter className="mt-4">
              <Button type="button" variant="outline" onClick={() => setRenameItem(null)}>
                Cancel
              </Button>
              <Button type="submit" disabled={actionBusy || !renameDraft.trim()}>
                {actionBusy ? "Saving…" : "Save"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={Boolean(deleteItem)} onOpenChange={(open) => !open && setDeleteItem(null)}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Delete {deleteItem?.name}?</DialogTitle>
            <DialogDescription>
              {deleteItem?.kind === "group"
                ? "This removes the group conversation and its transcript."
                : BOT_DELETE_COPY}
            </DialogDescription>
          </DialogHeader>
          {actionError ? (
            <p className="text-sm text-destructive" role="alert">{actionError}</p>
          ) : null}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setDeleteItem(null)}>
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
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
