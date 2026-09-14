"use client";

import {
  cloudHostErrorMessage,
  cloudHostFetch,
  readCloudApiErrorBody,
} from "@/lib/cloud-api";
import type { WorkspaceOverview } from "@/lib/workspace-types";
import { useCallback, useEffect, useRef, useState } from "react";

const MAX_ERROR_POLL_MS = 30_000;

export type WorkspaceLoadPhase = "initial" | "loading" | "ready" | "stale" | "unavailable";

export function useWorkspaceOverview(pollMs = 5000) {
  const [data, setData] = useState<WorkspaceOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [phase, setPhase] = useState<WorkspaceLoadPhase>("initial");
  const consecutiveFailures = useRef(0);
  const hasLoadedOnce = useRef(false);

  const refresh = useCallback(async (signal?: AbortSignal) => {
    setPhase(hasLoadedOnce.current ? "loading" : "initial");
    try {
      const response = await cloudHostFetch("/v1/workspace", {
        signal,
        timeoutMs: 20_000,
      });
      if (!response.ok) {
        const body = await readCloudApiErrorBody(response);
        throw new Error(
          body?.error?.trim() ||
            (await cloudHostErrorMessage(response, "Could not refresh your workspace")),
        );
      }
      const next: WorkspaceOverview = await response.json();
      consecutiveFailures.current = 0;
      hasLoadedOnce.current = true;
      setData(next);
      setError(null);
      setPhase("ready");
      return next;
    } catch (err) {
      if (signal?.aborted) {
        return null;
      }
      consecutiveFailures.current += 1;
      const message = err instanceof Error ? err.message : "Workspace unavailable";
      setError(message);
      setPhase(hasLoadedOnce.current ? "stale" : "unavailable");
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

  const runnerUnreachable =
    phase === "unavailable" || phase === "stale"
      ? error?.includes("temporarily unreachable") ?? false
      : false;

  return { data, error, phase, runnerUnreachable, refresh };
}
