"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import {
  sortWorkspaceEntries,
  WORKSPACE_ROOT,
  type WorkspaceEntry,
  type WorkspaceListResponse,
} from "@/lib/computer-workspace";
import {
  filterPublicWorkspaceEntries,
  pathsToRefreshOnInvalidation,
  workspaceRevisionChanged,
} from "@/lib/workspace-refresh";
import { useCallback, useEffect, useRef, useState } from "react";

interface WorkspaceListJson {
  path: string;
  revision?: number;
  entries: Array<{
    name: string;
    path: string;
    isDir: boolean;
  }>;
}

function normalizeEntries(raw: WorkspaceListJson["entries"]): WorkspaceEntry[] {
  return sortWorkspaceEntries(
    filterPublicWorkspaceEntries(
      raw.map((entry) => ({
        name: entry.name,
        path: entry.path,
        isDir: entry.isDir,
      })),
    ),
  );
}

const FETCH_TIMEOUT_MS = 90_000;
const REVISION_POLL_MS = 5_000;

function isAbortError(err: unknown): boolean {
  return err instanceof Error && err.name === "AbortError";
}

export function useComputerWorkspace(
  computerId: string | null,
  refreshGeneration = 0,
  options?: { pollRevision?: boolean },
) {
  const pollRevision = options?.pollRevision ?? true;
  const [dirs, setDirs] = useState<Record<string, WorkspaceEntry[]>>({});
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(() => new Set());
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [rootError, setRootError] = useState<string | null>(null);
  const dirsRef = useRef(dirs);
  dirsRef.current = dirs;
  const inflightSeqRef = useRef<Record<string, number>>({});
  const inflightPromiseRef = useRef<Record<string, Promise<void>>>({});
  const refreshAllPromiseRef = useRef<Promise<void> | null>(null);
  const computerIdRef = useRef(computerId);
  computerIdRef.current = computerId;
  const revisionRef = useRef<number | null>(null);
  const pollRevisionRef = useRef(pollRevision);
  pollRevisionRef.current = pollRevision;

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
      const existing = inflightPromiseRef.current[path];
      if (existing && !force) {
        return existing;
      }

      const run = async () => {
        const seq = (inflightSeqRef.current[path] ?? 0) + 1;
        inflightSeqRef.current[path] = seq;
        const controller = new AbortController();
        const timeoutId = window.setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);

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
            { signal: controller.signal },
          );
          if (
            computerIdRef.current !== computerId ||
            inflightSeqRef.current[path] !== seq
          ) {
            return;
          }
          const body = await response.json().catch(() => ({}));
          if (!response.ok) {
            const message =
              typeof body.error === "string" ? body.error : "Could not load folder";
            throw new Error(message);
          }
          const data = body as WorkspaceListResponse;
          if (data.revision !== undefined) {
            revisionRef.current = data.revision;
          }
          setDirs((prev) => ({
            ...prev,
            [path]: normalizeEntries(data.entries),
          }));
          if (path === WORKSPACE_ROOT) {
            setRootError(null);
          }
        } catch (err) {
          if (
            computerIdRef.current !== computerId ||
            inflightSeqRef.current[path] !== seq
          ) {
            return;
          }
          const message = isAbortError(err)
            ? "Timed out loading folder. Your computer may still be starting — try Refresh."
            : err instanceof Error
              ? err.message
              : "Could not load folder";
          setErrors((prev) => ({ ...prev, [path]: message }));
          if (path === WORKSPACE_ROOT) {
            setRootError(message);
          }
        } finally {
          window.clearTimeout(timeoutId);
          if (inflightSeqRef.current[path] === seq) {
            setPathLoading(path, false);
          }
        }
      };

      const promise = run().finally(() => {
        if (inflightPromiseRef.current[path] === promise) {
          delete inflightPromiseRef.current[path];
        }
      });
      inflightPromiseRef.current[path] = promise;
      return promise;
    },
    [computerId, setPathLoading],
  );

  const refreshLoaded = useCallback(() => {
    if (refreshAllPromiseRef.current) {
      return refreshAllPromiseRef.current;
    }
    const paths = pathsToRefreshOnInvalidation(
      Object.keys(dirsRef.current),
      WORKSPACE_ROOT,
    );
    const run = async () => {
      await Promise.all(paths.map((path) => loadDir(path, { force: true })));
    };
    const promise = run().finally(() => {
      if (refreshAllPromiseRef.current === promise) {
        refreshAllPromiseRef.current = null;
      }
    });
    refreshAllPromiseRef.current = promise;
    return promise;
  }, [loadDir]);

  const refresh = useCallback(() => {
    setErrors({});
    setRootError(null);
    void refreshLoaded();
  }, [refreshLoaded]);

  const probeRevision = useCallback(async () => {
    if (!computerId || !pollRevisionRef.current) {
      return;
    }
    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), 15_000);
    try {
      const response = await cloudHostFetch(
        `/v1/computers/${encodeURIComponent(computerId)}/workspace-revision`,
        { signal: controller.signal },
      );
      if (!response.ok || computerIdRef.current !== computerId) {
        return;
      }
      const body = (await response.json().catch(() => ({}))) as { revision?: number };
      if (workspaceRevisionChanged(revisionRef.current, body.revision)) {
        revisionRef.current = body.revision ?? revisionRef.current;
        await refreshLoaded();
      } else if (body.revision !== undefined && revisionRef.current === null) {
        revisionRef.current = body.revision;
      }
    } catch {
      /* passive probe */
    } finally {
      window.clearTimeout(timeoutId);
    }
  }, [computerId, refreshLoaded]);

  useEffect(() => {
    if (!computerId || refreshGeneration === 0) {
      return;
    }
    void refreshLoaded();
  }, [computerId, refreshGeneration, refreshLoaded]);

  useEffect(() => {
    if (!computerId || !pollRevision) {
      return;
    }
    const timer = window.setInterval(() => {
      if (document.visibilityState === "hidden") {
        return;
      }
      void probeRevision();
    }, REVISION_POLL_MS);
    return () => window.clearInterval(timer);
  }, [computerId, pollRevision, probeRevision]);

  useEffect(() => {
    inflightSeqRef.current = {};
    inflightPromiseRef.current = {};
    refreshAllPromiseRef.current = null;
    revisionRef.current = null;
    setDirs({});
    setErrors({});
    setRootError(null);
    setLoadingPaths(new Set());
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
    refreshLoaded,
    isLoading,
    errors,
    rootError,
  };
}
