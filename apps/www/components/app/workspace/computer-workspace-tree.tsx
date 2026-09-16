"use client";

import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { ComputerWorkspaceFileDialog } from "./computer-workspace-file-dialog";
import { Button } from "@/components/ui/button";
import {
  CollapseButton,
  File,
  Folder,
  Tree,
  useTree,
  type TreeViewElement,
} from "@/components/ui/file-tree";
import { Spinner } from "@/components/ui/spinner";
import { InlineRenameLabel } from "@/components/app/inline-rename-label";
import { Delete, RefreshCw, SquarePen } from "@/components/icons/lucide";
import { useActiveRun } from "@/contexts/active-run-context";
import { useComputerWorkspace } from "@/hooks/use-computer-workspace";
import {
  WORKSPACE_ROOT,
  type WorkspaceEntry,
} from "@/lib/computer-workspace";
import {
  deleteWorkspaceEntry,
  renameWorkspaceEntry,
} from "@/lib/computer-workspace-mutations";
import { cn } from "cn";
import { useCallback, useEffect, useMemo, useState } from "react";

interface ComputerWorkspaceTreeProps {
  computerId: string | null;
  className?: string;
}

interface ContextMenuState {
  entry: WorkspaceEntry;
  x: number;
  y: number;
}

const menuItemClass =
  "flex w-full cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground focus-visible:bg-accent";

function WorkspaceRowActions({
  name,
  hidden,
  onRename,
  onDelete,
}: {
  name: string;
  hidden?: boolean;
  onRename: () => void;
  onDelete: () => void;
}) {
  return (
    <div
      className={cn(
        "absolute top-0 right-0 z-10 flex h-7 items-center bg-gradient-to-l from-card from-40% via-card/90 to-transparent pl-4 pr-0.5 transition-opacity",
        hidden
          ? "pointer-events-none opacity-0"
          : "pointer-events-none opacity-0 group-hover/entry:pointer-events-auto group-hover/entry:opacity-100 group-focus-within/entry:pointer-events-auto group-focus-within/entry:opacity-100",
      )}
    >
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        className="size-6 text-muted-foreground"
        aria-label={`Rename ${name}`}
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onRename();
        }}
      >
        <SquarePen className="size-3.5" aria-hidden />
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        className="size-6 text-muted-foreground hover:text-destructive"
        aria-label={`Delete ${name}`}
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onDelete();
        }}
      >
        <Delete className="size-3.5" aria-hidden />
      </Button>
    </div>
  );
}

function workspaceEntriesToTreeElements(
  entries: WorkspaceEntry[],
  dirs: Record<string, WorkspaceEntry[]>,
): TreeViewElement[] {
  return entries.map((entry) => {
    if (!entry.isDir) {
      return {
        id: entry.path,
        name: entry.name,
        type: "file",
      };
    }
    const children = dirs[entry.path];
    return {
      id: entry.path,
      name: entry.name,
      type: "folder",
      children: children ? workspaceEntriesToTreeElements(children, dirs) : [],
    };
  });
}

function WorkspaceTreeExpansionLoader({
  loadDir,
}: {
  loadDir: (path: string) => void;
}) {
  const { expandedItems } = useTree();

  useEffect(() => {
    for (const path of expandedItems ?? []) {
      void loadDir(path);
    }
  }, [expandedItems, loadDir]);

  return null;
}

/** Compact card height when nothing is expanded; grow with the tree when folders open. */
function WorkspaceTreeCompactSync({
  onCompactChange,
}: {
  onCompactChange: (compact: boolean) => void;
}) {
  const { expandedItems } = useTree();

  useEffect(() => {
    onCompactChange((expandedItems?.length ?? 0) === 0);
  }, [expandedItems, onCompactChange]);

  return null;
}

function WorkspaceEntryName({
  entry,
  computerId,
  onRenamed,
  startEditing,
  onEditingChange,
  className,
}: {
  entry: WorkspaceEntry;
  computerId: string;
  onRenamed: () => void;
  startEditing?: boolean;
  onEditingChange?: (editing: boolean) => void;
  className?: string;
}) {
  return (
    <InlineRenameLabel
      value={entry.name}
      startEditing={startEditing}
      onEditingChange={onEditingChange}
      onCommit={async (next) => {
        await renameWorkspaceEntry(computerId, entry.path, next);
        onRenamed();
      }}
      className={cn("truncate text-xs font-normal", className)}
      inputClassName="text-xs font-normal"
      ariaLabel={`Rename ${entry.name}`}
      nested
    />
  );
}

function WorkspaceTreeBranch({
  entry,
  computerId,
  dirs,
  errors,
  isLoading,
  onOpenFile,
  onContextMenu,
  onRequestDelete,
  onRefresh,
  renamingPath,
  onRenamingPathChange,
}: {
  entry: WorkspaceEntry;
  computerId: string;
  dirs: Record<string, WorkspaceEntry[]>;
  errors: Record<string, string>;
  isLoading: (path: string) => boolean;
  onOpenFile: (path: string) => void;
  onContextMenu: (event: React.MouseEvent, entry: WorkspaceEntry) => void;
  onRequestDelete: (entry: WorkspaceEntry) => void;
  onRefresh: () => void;
  renamingPath: string | null;
  onRenamingPathChange: (path: string | null) => void;
}) {
  const handleContextMenu = useCallback(
    (event: React.MouseEvent) => {
      onContextMenu(event, entry);
    },
    [entry, onContextMenu],
  );

  const isRenaming = renamingPath === entry.path;
  const rowActions = (
    <WorkspaceRowActions
      name={entry.name}
      hidden={isRenaming}
      onRename={() => onRenamingPathChange(entry.path)}
      onDelete={() => onRequestDelete(entry)}
    />
  );

  if (!entry.isDir) {
    return (
      <div className="group/entry relative min-w-0">
        <File
          value={entry.path}
          handleSelect={onOpenFile}
          onContextMenu={handleContextMenu}
          aria-label={`Open ${entry.name}`}
          className="min-w-0"
        >
          <WorkspaceEntryName
            entry={entry}
            computerId={computerId}
            onRenamed={onRefresh}
            startEditing={isRenaming}
            onEditingChange={(editing) => {
              if (!editing && renamingPath === entry.path) {
                onRenamingPathChange(null);
              }
            }}
          />
        </File>
        {rowActions}
      </div>
    );
  }

  const children = dirs[entry.path];
  const loading = isLoading(entry.path);
  const error = errors[entry.path];

  return (
    <div className="group/entry relative min-w-0">
      <Folder
        className="min-w-0"
        value={entry.path}
        element={
          <WorkspaceEntryName
            entry={entry}
            computerId={computerId}
            onRenamed={onRefresh}
            className="font-medium"
            startEditing={isRenaming}
            onEditingChange={(editing) => {
              if (!editing && renamingPath === entry.path) {
                onRenamingPathChange(null);
              }
            }}
          />
        }
        onContextMenu={handleContextMenu}
      >
      {loading && !children ? (
        <div className="flex items-center gap-2 py-1 pl-1 text-[11px] text-muted-foreground">
          <Spinner className="size-3.5" />
          Loading…
        </div>
      ) : null}
      {error ? (
        <p className="py-1 pl-1 text-[11px] text-destructive" role="alert">
          {error}
        </p>
      ) : null}
      {children?.map((child) => (
        <WorkspaceTreeBranch
          key={child.path}
          entry={child}
          computerId={computerId}
          dirs={dirs}
          errors={errors}
          isLoading={isLoading}
          onOpenFile={onOpenFile}
          onContextMenu={onContextMenu}
          onRequestDelete={onRequestDelete}
          onRefresh={onRefresh}
          renamingPath={renamingPath}
          onRenamingPathChange={onRenamingPathChange}
        />
      ))}
      {children && children.length === 0 && !loading ? (
        <p className="py-1 pl-1 text-[11px] text-muted-foreground">Empty folder</p>
      ) : null}
      </Folder>
      {rowActions}
    </div>
  );
}

export function ComputerWorkspaceTree({ computerId, className }: ComputerWorkspaceTreeProps) {
  const { workspaceRefreshGeneration } = useActiveRun();
  const { dirs, loadDir, refresh, isLoading, errors, rootError } = useComputerWorkspace(
    computerId,
    workspaceRefreshGeneration,
  );
  const [openFilePath, setOpenFilePath] = useState<string | null>(null);
  const [fileDialogOpen, setFileDialogOpen] = useState(false);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [renamingPath, setRenamingPath] = useState<string | null>(null);
  const [deleteEntry, setDeleteEntry] = useState<WorkspaceEntry | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [treeCompact, setTreeCompact] = useState(true);

  const handleTreeCompactChange = useCallback((compact: boolean) => {
    setTreeCompact(compact);
  }, []);

  const handleOpenFile = useCallback((path: string) => {
    setOpenFilePath(path);
    setFileDialogOpen(true);
  }, []);

  const loadDirStable = useCallback(
    (path: string) => {
      void loadDir(path);
    },
    [loadDir],
  );

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

  const handleContextMenu = useCallback(
    (event: React.MouseEvent, entry: WorkspaceEntry) => {
      event.preventDefault();
      event.stopPropagation();
      setContextMenu({ entry, x: event.clientX, y: event.clientY });
    },
    [],
  );

  async function handleDeleteConfirm() {
    if (!deleteEntry || !computerId || actionBusy) {
      return;
    }
    setActionBusy(true);
    setActionError(null);
    try {
      await deleteWorkspaceEntry(computerId, deleteEntry.path);
      setDeleteEntry(null);
      if (openFilePath === deleteEntry.path) {
        setFileDialogOpen(false);
        setOpenFilePath(null);
      }
      refresh();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Could not delete");
    } finally {
      setActionBusy(false);
    }
  }

  const rootEntries = dirs[WORKSPACE_ROOT];

  const collapseElements = useMemo(
    () => (rootEntries ? workspaceEntriesToTreeElements(rootEntries, dirs) : []),
    [dirs, rootEntries],
  );

  if (!computerId) {
    return null;
  }

  const treeHeader = (
    <div className="flex items-center justify-between gap-2 border-b border-border/40 px-1 pb-1">
      <div className="flex min-w-0 items-center gap-1">
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="size-7 shrink-0 text-muted-foreground"
          onClick={() => refresh()}
          aria-label="Refresh workspace files"
        >
          <RefreshCw className="size-3.5" aria-hidden />
        </Button>
        <span className="truncate text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
          Workspace
        </span>
      </div>
      {collapseElements.length > 0 ? <CollapseButton elements={collapseElements} /> : null}
    </div>
  );

  return (
    <div className={cn("relative min-w-0", className)}>
      {rootError ? (
        <p className="px-0.5 text-xs text-destructive" role="alert">
          {rootError}
        </p>
      ) : null}

      {isLoading(WORKSPACE_ROOT) && !rootEntries ? (
        <div className="flex items-center gap-2 px-0.5 py-2 text-xs text-muted-foreground">
          <Spinner className="size-4" />
          Loading workspace…
        </div>
      ) : null}

      <div
        className={cn(
          "min-w-0 rounded-lg border border-border bg-card py-1 transition-[max-height] duration-200 ease-out",
          treeCompact
            ? "max-h-64 min-h-[8rem] overflow-y-auto overflow-x-hidden"
            : "overflow-x-hidden",
        )}
        aria-label="Workspace file tree"
      >
        {rootEntries && rootEntries.length > 0 ? (
          <Tree
            scrollable={treeCompact}
            indicator
            initialExpandedItems={[]}
            header={treeHeader}
          >
            <WorkspaceTreeCompactSync onCompactChange={handleTreeCompactChange} />
            <WorkspaceTreeExpansionLoader loadDir={loadDirStable} />
            {rootEntries.map((entry) => (
              <WorkspaceTreeBranch
                key={entry.path}
                entry={entry}
                computerId={computerId}
                dirs={dirs}
                errors={errors}
                isLoading={isLoading}
                onOpenFile={handleOpenFile}
                onContextMenu={handleContextMenu}
                onRequestDelete={(item) => {
                  setDeleteEntry(item);
                  setActionError(null);
                }}
                onRefresh={refresh}
                renamingPath={renamingPath}
                onRenamingPathChange={setRenamingPath}
              />
            ))}
          </Tree>
        ) : null}
        {rootEntries && rootEntries.length === 0 ? (
          <div>
            <div className="flex items-center gap-1 border-b border-border/40 px-1 pb-1">
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                className="size-7 shrink-0 text-muted-foreground"
                onClick={() => refresh()}
                aria-label="Refresh workspace files"
              >
                <RefreshCw className="size-3.5" aria-hidden />
              </Button>
              <span className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                Workspace
              </span>
            </div>
            <p className="px-3 py-4 text-center text-xs text-muted-foreground">
              Workspace is empty. Files appear here when your bot creates them.
            </p>
          </div>
        ) : null}
      </div>

      {contextMenu ? (
        <div
          className="fixed z-[100] min-w-40 rounded-lg border border-border/80 bg-popover p-1 text-popover-foreground shadow-lg ring-1 ring-foreground/10"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          role="menu"
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          <button
            type="button"
            role="menuitem"
            className={menuItemClass}
            onClick={() => {
              setRenamingPath(contextMenu.entry.path);
              closeContextMenu();
            }}
          >
            <SquarePen className="size-3.5" aria-hidden />
            Rename
          </button>
          <button
            type="button"
            role="menuitem"
            className={cn(menuItemClass, "text-destructive hover:bg-destructive/10 hover:text-destructive")}
            onClick={() => {
              setDeleteEntry(contextMenu.entry);
              setActionError(null);
              closeContextMenu();
            }}
          >
            <Delete className="size-3.5" aria-hidden />
            Delete
          </button>
        </div>
      ) : null}

      <ConfirmAlertDialog
        open={Boolean(deleteEntry)}
        onOpenChange={(open) => {
          if (!open && !actionBusy) {
            setDeleteEntry(null);
            setActionError(null);
          }
        }}
        title={deleteEntry ? `Delete ${deleteEntry.name}?` : "Delete?"}
        description={
          deleteEntry?.isDir
            ? "This removes the folder and everything inside it from the bot workspace."
            : "This removes the file from the bot workspace."
        }
        confirmLabel="Delete"
        pendingLabel="Deleting…"
        destructive
        pending={actionBusy}
        error={actionError}
        onConfirm={handleDeleteConfirm}
      />

      <ComputerWorkspaceFileDialog
        computerId={computerId}
        path={openFilePath}
        open={fileDialogOpen}
        onOpenChange={setFileDialogOpen}
      />
    </div>
  );
}
