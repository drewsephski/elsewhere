"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { WorkspaceOverview } from "@/lib/workspace-types";
import { useCallback, useEffect, useState } from "react";

export function useWorkspaceOverview(pollMs = 5000) {
  const [data, setData] = useState<WorkspaceOverview | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async (signal?: AbortSignal) => {
    try {
      const response = await cloudHostFetch("/v1/workspace", { signal });
      if (!response.ok) {
        throw new Error("Could not refresh your workspace");
      }
      const next: WorkspaceOverview = await response.json();
      setData(next);
      setError(null);
      return next;
    } catch (err) {
      if (signal?.aborted) {
        return null;
      }
      setError(err instanceof Error ? err.message : "Workspace unavailable");
      return null;
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;

    async function tick() {
      await refresh(controller.signal);
      if (!controller.signal.aborted) {
        timer = setTimeout(() => void tick(), pollMs);
      }
    }

    void tick();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [pollMs, refresh]);

  return { data, error, refresh };
}
