"use client";

import { cloudHostEventStream } from "@/lib/cloud-api";
import type { RunTimelineState } from "@/lib/run-event-timeline";
import { emptyRunTimelineState, replayRunStreamEvents } from "@/lib/run-event-timeline";
import { useEffect, useState } from "react";

const timelineCache = new Map<string, RunTimelineState>();
const inflight = new Map<string, Promise<RunTimelineState>>();

function runIsTerminal(status: string): boolean {
  return status !== "queued" && status !== "running";
}

async function loadTimelineFromEvents(runId: string, signal?: AbortSignal): Promise<RunTimelineState> {
  const cached = timelineCache.get(runId);
  if (cached) {
    return cached;
  }
  const existing = inflight.get(runId);
  if (existing) {
    return existing;
  }

  const promise = (async () => {
    const collected: { id?: string; event: string; data: string }[] = [];
    await cloudHostEventStream(`/v1/runs/${runId}/events`, {
      signal,
      onEvent(event) {
        collected.push(event);
      },
    });
    const state = replayRunStreamEvents(runId, collected);
    timelineCache.set(runId, state);
    inflight.delete(runId);
    return state;
  })();

  inflight.set(runId, promise);
  try {
    return await promise;
  } catch (error) {
    inflight.delete(runId);
    throw error;
  }
}

/** Clears cached replay when a run is live again (new events will merge via SSE). */
export function invalidateHistoricalRunTimeline(runId: string): void {
  timelineCache.delete(runId);
}

export function useHistoricalRunTimelines(
  runIds: string[],
  statuses: Record<string, string>,
  liveRunId: string | null,
): Record<string, RunTimelineState> {
  const [timelines, setTimelines] = useState<Record<string, RunTimelineState>>({});

  useEffect(() => {
    const terminalIds = runIds.filter(
      (id) => id !== liveRunId && runIsTerminal(statuses[id] ?? ""),
    );
    if (terminalIds.length === 0) {
      return;
    }

    const controller = new AbortController();
    void (async () => {
      const next: Record<string, RunTimelineState> = {};
      for (const runId of terminalIds) {
        const cached = timelineCache.get(runId);
        if (cached) {
          next[runId] = cached;
          continue;
        }
        try {
          next[runId] = await loadTimelineFromEvents(runId, controller.signal);
        } catch {
          if (!controller.signal.aborted) {
            next[runId] = emptyRunTimelineState();
          }
        }
      }
      if (!controller.signal.aborted) {
        setTimelines((previous) => ({ ...previous, ...next }));
      }
    })();

    return () => controller.abort();
  }, [runIds.join(","), liveRunId, JSON.stringify(statuses)]);

  return timelines;
}
