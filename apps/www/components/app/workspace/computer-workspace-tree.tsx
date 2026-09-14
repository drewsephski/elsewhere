"use client";

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { ChevronRight, FileText, Files, RefreshCw } from "@/components/icons/lucide";
import { useComputerWorkspace } from "@/hooks/use-computer-workspace";
import {
  WORKSPACE_ROOT,
  type WorkspaceEntry,
} from "@/lib/computer-workspace";
import { cn } from "cn";
import { useCallback, useState } from "react";
import { ComputerWorkspaceFileDialog } from "./computer-workspace-file-dialog";

interface ComputerWorkspaceTreeProps {
  computerId: string | null;
  className?: string;
}

function WorkspaceTreeNode({
  computerId,
  entry,
  depth,
  dirs,
  errors,
  isLoading,
  loadDir,
  onOpenFile,
}: {
  computerId: string;
  entry: WorkspaceEntry;
  depth: number;
  dirs: Record<string, WorkspaceEntry[]>;
  errors: Record<string, string>;
  isLoading: (path: string) => boolean;
  loadDir: (path: string) => void;
  onOpenFile: (path: string) => void;
}) {
  const [open, setOpen] = useState(false);

  const handleOpenChange = useCallback(
    (next: boolean) => {
      setOpen(next);
      if (next && entry.isDir) {
        void loadDir(entry.path);
      }
    },
    [entry.isDir, entry.path, loadDir],
  );

  if (!entry.isDir) {
    return (
      <button
        type="button"
        onClick={() => onOpenFile(entry.path)}
        className="flex w-full min-w-0 items-center gap-1.5 rounded-md py-1 pr-1 text-left text-xs text-foreground/90 transition-colors hover:bg-white/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/25"
        style={{ paddingLeft: `${depth * 12 + 4}px` }}
        aria-label={`Open ${entry.name}`}
      >
        <FileText className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
        <span className="truncate">{entry.name}</span>
      </button>
    );
  }

  const children = dirs[entry.path];
  const loading = isLoading(entry.path);
  const error = errors[entry.path];

  return (
    <Collapsible open={open} onOpenChange={handleOpenChange} className="min-w-0">
      <CollapsibleTrigger
        className="flex w-full min-w-0 cursor-pointer items-center gap-1 rounded-md py-1 pr-1 text-left text-xs font-medium text-foreground transition-colors hover:bg-white/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/25"
        style={{ paddingLeft: `${depth * 12 + 4}px` }}
        aria-label={`${open ? "Collapse" : "Expand"} folder ${entry.name}`}
      >
        <ChevronRight
          className={cn(
            "size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 ease-out",
            open && "rotate-90",
          )}
          aria-hidden
        />
        <Files className="size-3.5 shrink-0 text-amber-600/90" aria-hidden />
        <span className="truncate">{entry.name}</span>
      </CollapsibleTrigger>
      <CollapsibleContent
        className="overflow-hidden duration-200 data-closed:animate-out data-open:animate-in data-closed:fade-out-0 data-open:fade-in-0"
      >
        <div className="border-l border-border/50" style={{ marginLeft: `${depth * 12 + 10}px` }}>
          {loading && !children ? (
            <div className="flex items-center gap-2 py-1.5 pl-3 text-[11px] text-muted-foreground">
              <Spinner className="size-3.5" />
              Loading…
            </div>
          ) : null}
          {error ? (
            <p className="py-1.5 pl-3 text-[11px] text-red-600" role="alert">
              {error}
            </p>
          ) : null}
          {children?.map((child) => (
            <WorkspaceTreeNode
              key={child.path}
              computerId={computerId}
              entry={child}
              depth={depth + 1}
              dirs={dirs}
              errors={errors}
              isLoading={isLoading}
              loadDir={loadDir}
              onOpenFile={onOpenFile}
            />
          ))}
          {children && children.length === 0 && !loading ? (
            <p className="py-1.5 pl-3 text-[11px] text-muted-foreground">Empty folder</p>
          ) : null}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

export function ComputerWorkspaceTree({ computerId, className }: ComputerWorkspaceTreeProps) {
  const { dirs, loadDir, refresh, isLoading, errors, rootError } =
    useComputerWorkspace(computerId);
  const [openFilePath, setOpenFilePath] = useState<string | null>(null);
  const [fileDialogOpen, setFileDialogOpen] = useState(false);

  const handleOpenFile = useCallback((path: string) => {
    setOpenFilePath(path);
    setFileDialogOpen(true);
  }, []);

  const rootEntries = dirs[WORKSPACE_ROOT];

  if (!computerId) {
    return null;
  }

  return (
    <div className={cn("min-w-0", className)}>
      <div className="mb-2 flex items-center justify-between gap-2 px-1">
        <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
          /workspace
        </p>
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

      {rootError ? (
        <p className="px-1 text-xs text-red-600" role="alert">
          {rootError}
        </p>
      ) : null}

      {isLoading(WORKSPACE_ROOT) && !rootEntries ? (
        <div className="flex items-center gap-2 px-1 py-2 text-xs text-muted-foreground">
          <Spinner className="size-4" />
          Loading workspace…
        </div>
      ) : null}

      <div className="max-h-52 min-w-0 overflow-y-auto rounded-lg border border-border/60 bg-white/50 px-0.5 py-1">
        {rootEntries?.map((entry) => (
          <WorkspaceTreeNode
            key={entry.path}
            computerId={computerId}
            entry={entry}
            depth={0}
            dirs={dirs}
            errors={errors}
            isLoading={isLoading}
            loadDir={(path) => void loadDir(path)}
            onOpenFile={handleOpenFile}
          />
        ))}
        {rootEntries && rootEntries.length === 0 ? (
          <p className="px-2 py-3 text-center text-xs text-muted-foreground">
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
