"use client";

import { ComputerWorkspaceFileDialog } from "./computer-workspace-file-dialog";
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
  CollapseButton,
  File,
  Folder,
  Tree,
  useTree,
  type TreeViewElement,
} from "@/components/ui/file-tree";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { RefreshCw } from "@/components/icons/lucide";
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
  "flex w-full cursor-default items-center rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground focus-visible:bg-accent";

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

function WorkspaceTreeBranch({
  entry,
  dirs,
  errors,
  isLoading,
  onOpenFile,
  onContextMenu,
}: {
  entry: WorkspaceEntry;
  dirs: Record<string, WorkspaceEntry[]>;
  errors: Record<string, string>;
  isLoading: (path: string) => boolean;
  onOpenFile: (path: string) => void;
  onContextMenu: (event: React.MouseEvent, entry: WorkspaceEntry) => void;
}) {
  const handleContextMenu = useCallback(
    (event: React.MouseEvent) => {
      onContextMenu(event, entry);
    },
    [entry, onContextMenu],
  );

  if (!entry.isDir) {
    return (
      <File
        value={entry.path}
        handleSelect={onOpenFile}
        onContextMenu={handleContextMenu}
        aria-label={`Open ${entry.name}`}
      >
        {entry.name}
      </File>
    );
  }

  const children = dirs[entry.path];
  const loading = isLoading(entry.path);
  const error = errors[entry.path];

  return (
    <Folder value={entry.path} element={entry.name} onContextMenu={handleContextMenu}>
      {loading && !children ? (
        <div className="flex items-center gap-2 py-1 pl-1 text-[11px] text-muted-foreground">
          <Spinner className="size-3.5" />
          Loading…
        </div>
      ) : null}
      {error ? (
        <p className="py-1 pl-1 text-[11px] text-red-600" role="alert">
          {error}
        </p>
      ) : null}
      {children?.map((child) => (
        <WorkspaceTreeBranch
          key={child.path}
          entry={child}
          dirs={dirs}
          errors={errors}
          isLoading={isLoading}
          onOpenFile={onOpenFile}
          onContextMenu={onContextMenu}
        />
      ))}
      {children && children.length === 0 && !loading ? (
        <p className="py-1 pl-1 text-[11px] text-muted-foreground">Empty folder</p>
      ) : null}
    </Folder>
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
  const [renameEntry, setRenameEntry] = useState<WorkspaceEntry | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
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

  function openRenameDialog(entry: WorkspaceEntry) {
    setRenameEntry(entry);
    setRenameDraft(entry.name);
    setActionError(null);
  }

  async function handleRenameSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!renameEntry || !computerId || actionBusy) {
      return;
    }
    const trimmed = renameDraft.trim();
    if (!trimmed || trimmed === renameEntry.name) {
      setRenameEntry(null);
      return;
    }
    setActionBusy(true);
    setActionError(null);
    try {
      await renameWorkspaceEntry(computerId, renameEntry.path, trimmed);
      setRenameEntry(null);
      refresh();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Could not rename");
    } finally {
      setActionBusy(false);
    }
  }

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

  return (
    <div className={cn("relative min-w-0", className)}>
      <div className="mb-2 flex items-center justify-between gap-2 px-0.5">
        <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
          /workspace
        </p>
        <div className="flex items-center gap-0.5">
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
        </div>
      </div>

      {rootError ? (
        <p className="px-0.5 text-xs text-red-600" role="alert">
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
          "min-w-0 rounded-lg border border-border/60 bg-white/50 py-1 transition-[max-height] duration-200 ease-out",
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
            header={
              collapseElements.length > 0 ? (
                <div className="flex justify-end border-b border-border/40 px-1 pb-1">
                  <CollapseButton elements={collapseElements} />
                </div>
              ) : null
            }
          >
            <WorkspaceTreeCompactSync onCompactChange={handleTreeCompactChange} />
            <WorkspaceTreeExpansionLoader loadDir={loadDirStable} />
            {rootEntries.map((entry) => (
              <WorkspaceTreeBranch
                key={entry.path}
                entry={entry}
                dirs={dirs}
                errors={errors}
                isLoading={isLoading}
                onOpenFile={handleOpenFile}
                onContextMenu={handleContextMenu}
              />
            ))}
          </Tree>
        ) : null}
        {rootEntries && rootEntries.length === 0 ? (
          <p className="px-3 py-4 text-center text-xs text-muted-foreground">
            Workspace is empty. Files appear here when your bot creates them.
          </p>
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
              openRenameDialog(contextMenu.entry);
              closeContextMenu();
            }}
          >
            Rename
          </button>
          <button
            type="button"
            role="menuitem"
            className={cn(menuItemClass, "text-red-700 hover:bg-red-50 hover:text-red-800")}
            onClick={() => {
              setDeleteEntry(contextMenu.entry);
              setActionError(null);
              closeContextMenu();
            }}
          >
            Delete
          </button>
        </div>
      ) : null}

      <Dialog open={Boolean(renameEntry)} onOpenChange={(open) => !open && setRenameEntry(null)}>
        <DialogContent className="sm:max-w-sm">
          <form onSubmit={(event) => void handleRenameSubmit(event)}>
            <DialogHeader>
              <DialogTitle>Rename {renameEntry?.isDir ? "folder" : "file"}</DialogTitle>
              <DialogDescription>
                Enter a new name for &ldquo;{renameEntry?.name}&rdquo;.
              </DialogDescription>
            </DialogHeader>
            <div className="mt-4 space-y-2">
              <Label htmlFor="workspace-rename">Name</Label>
              <Input
                id="workspace-rename"
                value={renameDraft}
                onChange={(event) => setRenameDraft(event.target.value)}
                maxLength={255}
                required
                autoFocus
              />
            </div>
            {actionError ? (
              <p className="mt-2 text-sm text-red-700" role="alert">{actionError}</p>
            ) : null}
            <DialogFooter className="mt-4">
              <Button type="button" variant="outline" onClick={() => setRenameEntry(null)}>
                Cancel
              </Button>
              <Button type="submit" disabled={actionBusy || !renameDraft.trim()}>
                {actionBusy ? "Saving…" : "Save"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={Boolean(deleteEntry)} onOpenChange={(open) => !open && setDeleteEntry(null)}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Delete {deleteEntry?.name}?</DialogTitle>
            <DialogDescription>
              {deleteEntry?.isDir
                ? "This removes the folder and everything inside it from the bot workspace."
                : "This removes the file from the bot workspace."}
            </DialogDescription>
          </DialogHeader>
          {actionError ? (
            <p className="text-sm text-red-700" role="alert">{actionError}</p>
          ) : null}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setDeleteEntry(null)}>
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

      <ComputerWorkspaceFileDialog
        computerId={computerId}
        path={openFilePath}
        open={fileDialogOpen}
        onOpenChange={setFileDialogOpen}
      />
    </div>
  );
}
