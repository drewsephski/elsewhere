import { cloudHostEventStream } from "@/lib/cloud-api";
import type { RunTimelineState } from "@/lib/run-event-timeline";
import { emptyRunTimelineState, replayRunStreamEvents } from "@/lib/run-event-timeline";

const timelineCache = new Map<string, RunTimelineState>();
const inflight = new Map<string, Promise<RunTimelineState>>();

export function getCachedHistoricalTimeline(runId: string): RunTimelineState | undefined {
  return timelineCache.get(runId);
}

export function clearHistoricalTimelineCacheForTests(): void {
  timelineCache.clear();
  inflight.clear();
}

/** Clears cached replay when a run is live again (new events will merge via SSE). */
export function invalidateHistoricalRunTimeline(runId: string): void {
  timelineCache.delete(runId);
}

/**
 * Loads timeline from durable events. Inflight is shared without AbortSignal so one
 * consumer unmount cannot abort another subscriber to the same runId.
 */
export function loadHistoricalTimelineFromEvents(runId: string): Promise<RunTimelineState> {
  const cached = timelineCache.get(runId);
  if (cached) {
    return Promise.resolve(cached);
  }
  const existing = inflight.get(runId);
  if (existing) {
    return existing;
  }

  const promise = (async () => {
    const collected: { id?: string; event: string; data: string }[] = [];
    await cloudHostEventStream(`/v1/runs/${runId}/events`, {
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
  return promise.catch((error) => {
    inflight.delete(runId);
    throw error;
  });
}

export async function hydrateTerminalRunTimelinesProgressive(
  runIds: string[],
  onRunHydrated: (runId: string, state: RunTimelineState) => void,
  isCancelled: () => boolean,
): Promise<void> {
  for (const runId of runIds) {
    if (isCancelled()) {
      return;
    }
    const cached = timelineCache.get(runId);
    if (cached) {
      onRunHydrated(runId, cached);
      continue;
    }
    void loadHistoricalTimelineFromEvents(runId)
      .then((state) => {
        if (!isCancelled()) {
          onRunHydrated(runId, state);
        }
      })
      .catch(() => {
        if (!isCancelled()) {
          onRunHydrated(runId, emptyRunTimelineState());
        }
      });
  }
}
