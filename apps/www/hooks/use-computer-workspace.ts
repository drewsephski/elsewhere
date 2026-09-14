"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import {
  sortWorkspaceEntries,
  WORKSPACE_ROOT,
  type WorkspaceEntry,
  type WorkspaceListResponse,
} from "@/lib/computer-workspace";
import { useCallback, useEffect, useRef, useState } from "react";

interface WorkspaceListJson {
  path: string;
  entries: Array<{
    name: string;
    path: string;
    isDir: boolean;
  }>;
}

function normalizeEntries(raw: WorkspaceListJson["entries"]): WorkspaceEntry[] {
  return sortWorkspaceEntries(
    raw.map((entry) => ({
      name: entry.name,
      path: entry.path,
      isDir: entry.isDir,
    })),
  );
}

export function useComputerWorkspace(computerId: string | null) {
  const [dirs, setDirs] = useState<Record<string, WorkspaceEntry[]>>({});
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(() => new Set());
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [rootError, setRootError] = useState<string | null>(null);
  const dirsRef = useRef(dirs);
  dirsRef.current = dirs;

  const setPathLoading = useCallback((path: string, loading: boolean) => {
    setLoadingPaths((prev) => {
      const next = new Set(prev);
      if (loading) {
        next.add(path);
      } else {
        next.delete(path);
      }
      return next;
    });
  }, []);

  const loadDir = useCallback(
    async (path: string, { force = false }: { force?: boolean } = {}) => {
      if (!computerId) {
        return;
      }
      if (!force && dirsRef.current[path]) {
        return;
      }
      setPathLoading(path, true);
      setErrors((prev) => {
        if (!prev[path]) {
          return prev;
        }
        const next = { ...prev };
        delete next[path];
        return next;
      });
      try {
        const response = await cloudHostFetch(
          `/v1/computers/${encodeURIComponent(computerId)}/workspace?path=${encodeURIComponent(path)}`,
        );
        const body = await response.json().catch(() => ({}));
        if (!response.ok) {
          const message =
            typeof body.error === "string" ? body.error : "Could not load folder";
          throw new Error(message);
        }
        const data = body as WorkspaceListResponse;
        setDirs((prev) => ({
          ...prev,
          [path]: normalizeEntries(data.entries),
        }));
        if (path === WORKSPACE_ROOT) {
          setRootError(null);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "Could not load folder";
        setErrors((prev) => ({ ...prev, [path]: message }));
        if (path === WORKSPACE_ROOT) {
          setRootError(message);
        }
      } finally {
        setPathLoading(path, false);
      }
    },
    [computerId, setPathLoading],
  );

  const refresh = useCallback(() => {
    setDirs({});
    setErrors({});
    setRootError(null);
    void loadDir(WORKSPACE_ROOT, { force: true });
  }, [loadDir]);

  useEffect(() => {
    setDirs({});
    setErrors({});
    setRootError(null);
    if (!computerId) {
      return;
    }
    void loadDir(WORKSPACE_ROOT, { force: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- reset when computer changes only
  }, [computerId]);

  const isLoading = (path: string) => loadingPaths.has(path);

  return {
    dirs,
    loadDir,
    refresh,
    isLoading,
    errors,
    rootError,
  };
}
