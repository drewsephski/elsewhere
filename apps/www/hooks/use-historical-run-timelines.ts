"use client";

import type { RunTimelineState } from "@/lib/run-event-timeline";
import {
  getCachedHistoricalTimeline,
  hydrateTerminalRunTimelinesProgressive,
} from "@/lib/historical-run-timeline-loader";
import { useEffect, useState } from "react";

function runIsTerminal(status: string): boolean {
  return status !== "queued" && status !== "running";
}

export { invalidateHistoricalRunTimeline } from "@/lib/historical-run-timeline-loader";

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

    let cancelled = false;

    const seeded: Record<string, RunTimelineState> = {};
    for (const runId of terminalIds) {
      const cached = getCachedHistoricalTimeline(runId);
      if (cached) {
        seeded[runId] = cached;
      }
    }
    if (Object.keys(seeded).length > 0) {
      setTimelines((previous) => ({ ...previous, ...seeded }));
    }

    void hydrateTerminalRunTimelinesProgressive(
      terminalIds.filter((id) => !seeded[id]),
      (runId, state) => {
        setTimelines((previous) => ({ ...previous, [runId]: state }));
      },
      () => cancelled,
    );

    return () => {
      cancelled = true;
    };
  }, [runIds.join(","), liveRunId, JSON.stringify(statuses)]);

  return timelines;
}
