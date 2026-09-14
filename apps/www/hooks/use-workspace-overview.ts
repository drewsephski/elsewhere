"use client";

import { cloudHostErrorMessage, cloudHostFetch } from "@/lib/cloud-api";
import type { WorkspaceOverview } from "@/lib/workspace-types";
import { useCallback, useEffect, useRef, useState } from "react";

const MAX_ERROR_POLL_MS = 30_000;

export function useWorkspaceOverview(pollMs = 5000) {
  const [data, setData] = useState<WorkspaceOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const consecutiveFailures = useRef(0);

  const refresh = useCallback(async (signal?: AbortSignal) => {
    try {
      const response = await cloudHostFetch("/v1/workspace", {
        signal,
        timeoutMs: 20_000,
      });
      if (!response.ok) {
        throw new Error(
          await cloudHostErrorMessage(response, "Could not refresh your workspace"),
        );
      }
      const next: WorkspaceOverview = await response.json();
      consecutiveFailures.current = 0;
      setData(next);
      setError(null);
      return next;
    } catch (err) {
      if (signal?.aborted) {
        return null;
      }
      consecutiveFailures.current += 1;
      setError(err instanceof Error ? err.message : "Workspace unavailable");
      return null;
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;

    async function tick() {
      const next = await refresh(controller.signal);
      if (!controller.signal.aborted) {
        const delay = next
          ? pollMs
          : Math.min(
              MAX_ERROR_POLL_MS,
              pollMs * 2 ** Math.min(consecutiveFailures.current - 1, 3),
            );
        timer = setTimeout(() => void tick(), delay);
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
