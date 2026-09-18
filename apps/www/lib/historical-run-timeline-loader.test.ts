import { afterEach, describe, expect, it, vi } from "vitest";
import {
  clearHistoricalTimelineCacheForTests,
  hydrateTerminalRunTimelinesProgressive,
  loadHistoricalTimelineFromEvents,
} from "./historical-run-timeline-loader";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostEventStream: vi.fn(),
}));

import { cloudHostEventStream } from "@/lib/cloud-api";

const mockStream = vi.mocked(cloudHostEventStream);

afterEach(() => {
  clearHistoricalTimelineCacheForTests();
  mockStream.mockReset();
});

describe("loadHistoricalTimelineFromEvents", () => {
  it("shares inflight without tying to a consumer AbortSignal", async () => {
    let release: (() => void) | undefined;
    mockStream.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          release = resolve;
        }),
    );

    const first = loadHistoricalTimelineFromEvents("run_shared");
    const second = loadHistoricalTimelineFromEvents("run_shared");
    expect(mockStream).toHaveBeenCalledTimes(1);

    release?.();
    await Promise.all([first, second]);
    expect(mockStream).toHaveBeenCalledTimes(1);
  });
});

describe("hydrateTerminalRunTimelinesProgressive", () => {
  it("updates each run as soon as its replay finishes", async () => {
    const order: string[] = [];
    mockStream.mockImplementation(async (path: string) => {
      const runId = path.split("/")[3];
      if (runId === "run_slow") {
        await new Promise((resolve) => setTimeout(resolve, 30));
      }
      return undefined;
    });

    await new Promise<void>((resolve) => {
      void hydrateTerminalRunTimelinesProgressive(
        ["run_fast", "run_slow"],
        (runId) => {
          order.push(runId);
          if (order.length === 2) {
            resolve();
          }
        },
        () => false,
      );
    });

    expect(order[0]).toBe("run_fast");
    expect(order[1]).toBe("run_slow");
  });

  it("does not poison later consumers when an earlier hydration pass is cancelled", async () => {
    let release: (() => void) | undefined;
    mockStream.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          release = resolve;
        }),
    );

    const updates: string[] = [];
    let cancelled = false;
    void hydrateTerminalRunTimelinesProgressive(
      ["run_poison"],
      (runId) => updates.push(runId),
      () => cancelled,
    );
    cancelled = true;

    release?.();
    await loadHistoricalTimelineFromEvents("run_poison");

    const updatesAfter: string[] = [];
    await new Promise<void>((resolve) => {
      void hydrateTerminalRunTimelinesProgressive(
        ["run_poison"],
        (runId) => {
          updatesAfter.push(runId);
          resolve();
        },
        () => false,
      );
    });

    expect(updatesAfter).toEqual(["run_poison"]);
  });
});
