"use client";

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
import { RefreshCw } from "@/components/icons/lucide";
import { useActiveRun } from "@/contexts/active-run-context";
import { useComputerWorkspace } from "@/hooks/use-computer-workspace";
import {
  WORKSPACE_ROOT,
  type WorkspaceEntry,
} from "@/lib/computer-workspace";
import { cn } from "cn";
import { useCallback, useEffect, useMemo, useState } from "react";

interface ComputerWorkspaceTreeProps {
  computerId: string | null;
  className?: string;
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

function WorkspaceTreeBranch({
  entry,
  dirs,
  errors,
  isLoading,
  onOpenFile,
}: {
  entry: WorkspaceEntry;
  dirs: Record<string, WorkspaceEntry[]>;
  errors: Record<string, string>;
  isLoading: (path: string) => boolean;
  onOpenFile: (path: string) => void;
}) {
  if (!entry.isDir) {
    return (
      <File
        value={entry.path}
        handleSelect={onOpenFile}
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
    <Folder value={entry.path} element={entry.name}>
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
        className="max-h-64 min-h-[8rem] rounded-lg border border-border/60 bg-white/50 py-1"
        aria-label="Workspace file tree"
      >
        {rootEntries && rootEntries.length > 0 ? (
          <Tree
            className="h-full max-h-64"
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
            <WorkspaceTreeExpansionLoader loadDir={loadDirStable} />
            {rootEntries.map((entry) => (
              <WorkspaceTreeBranch
                key={entry.path}
                entry={entry}
                dirs={dirs}
                errors={errors}
                isLoading={isLoading}
                onOpenFile={handleOpenFile}
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

      <ComputerWorkspaceFileDialog
        computerId={computerId}
        path={openFilePath}
        open={fileDialogOpen}
        onOpenChange={setFileDialogOpen}
      />
    </div>
  );
}
