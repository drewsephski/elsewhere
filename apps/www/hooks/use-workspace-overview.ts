"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { cloudApiErrorFromResponse, isCloudApiError } from "@/lib/cloud-api-error";
import { formatUserFacingError } from "@/lib/format-api-error";
import type { WorkspaceOverview } from "@/lib/workspace-types";
import { useCallback, useEffect, useRef, useState } from "react";

const MAX_ERROR_POLL_MS = 30_000;

export type WorkspaceLoadPhase = "initial" | "loading" | "ready" | "stale" | "unavailable";

export function useWorkspaceOverview(pollMs = 5000) {
  const [data, setData] = useState<WorkspaceOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);
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
        throw await cloudApiErrorFromResponse(
          response,
          "Could not refresh your workspace",
        );
      }
      const next: WorkspaceOverview = await response.json();
      consecutiveFailures.current = 0;
      hasLoadedOnce.current = true;
      setData(next);
      setError(null);
      setErrorCode(null);
      setPhase("ready");
      return next;
    } catch (err) {
      if (signal?.aborted) {
        return null;
      }
      consecutiveFailures.current += 1;
      setError(formatUserFacingError(err, "Workspace unavailable"));
      setErrorCode(isCloudApiError(err) ? err.code ?? null : null);
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
    (phase === "unavailable" || phase === "stale") &&
    errorCode === "workspace_upstream_unreachable";

  return { data, error, phase, runnerUnreachable, refresh };
}
