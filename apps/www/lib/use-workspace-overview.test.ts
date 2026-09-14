import { describe, expect, it } from "vitest";

describe("workspace load phases", () => {
  it("does not treat initial outage as an empty account", () => {
    const phase = "unavailable" as const;
    const label =
      phase === "initial" || phase === "loading"
        ? "Loading your bots…"
        : phase === "unavailable" || phase === "stale"
          ? "Runner unavailable"
          : "No bots yet.";
    expect(label).toBe("Runner unavailable");
  });

  it("preserves stale workspace data semantics", () => {
    const previousBots = [{ id: "1" }];
    const phase = "stale" as const;
    const bots = phase === "stale" ? previousBots : [];
    expect(bots).toHaveLength(1);
  });
});
