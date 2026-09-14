/**
 * Coalesces overlapping browser preview fetches: at most one in flight plus one follow-up.
 */
export class BrowserPreviewFetchScheduler {
  inFlight = false;
  dirty = false;
  fetchCount = 0;

  async runCoalesced(fetchFn: () => Promise<void>): Promise<void> {
    if (this.inFlight) {
      this.dirty = true;
      return;
    }

    do {
      this.dirty = false;
      this.inFlight = true;
      try {
        this.fetchCount += 1;
        await fetchFn();
      } finally {
        this.inFlight = false;
      }
    } while (this.dirty);
  }

  reset(): void {
    this.inFlight = false;
    this.dirty = false;
  }
}
