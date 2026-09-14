import { describe, expect, it, vi } from "vitest";
import { BrowserPreviewFetchScheduler } from "./browser-preview-fetch-scheduler";

describe("BrowserPreviewFetchScheduler", () => {
  it("runs at most one follow-up when refresh is requested repeatedly during an active fetch", async () => {
    const scheduler = new BrowserPreviewFetchScheduler();
    let inFlight = 0;
    let maxConcurrent = 0;

    const fetchFn = vi.fn(async () => {
      inFlight += 1;
      maxConcurrent = Math.max(maxConcurrent, inFlight);
      await new Promise((resolve) => setTimeout(resolve, 20));
      inFlight -= 1;
    });

    const runs = Array.from({ length: 5 }, () =>
      scheduler.runCoalesced(fetchFn),
    );
    await Promise.all(runs);

    expect(fetchFn).toHaveBeenCalledTimes(2);
    expect(maxConcurrent).toBe(1);
    expect(scheduler.fetchCount).toBe(2);
  });
});
