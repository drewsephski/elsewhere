// @vitest-environment happy-dom

import type { RunSummary } from "@/lib/api-types";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RecentRunsPanel } from "./recent-runs-panel";

const longAssignment =
  "Research the following three pages using your browser tools: https://example.com/ https://www.iana.org/help/example-domains https://httpbin.org/forms/post Visit Example.com";

const runs: RunSummary[] = [
  {
    runId: "run_1",
    requestId: "req_1",
    botId: "bot_1",
    conversationId: "conv_1",
    task: longAssignment,
    botName: "Designer",
    model: "gpt-5.6-tuna",
    status: "completed",
    createdAt: "2026-09-15T12:00:00.000Z",
    startedAt: "2026-09-15T12:00:00.000Z",
    finishedAt: "2026-09-15T12:02:00.000Z",
    computerId: null,
  },
  {
    runId: "run_2",
    requestId: "req_2",
    botId: "bot_2",
    conversationId: "conv_2",
    task: "What test color did I tell you? Do not read the file; answer only from my conversation.",
    botName: "Project Manager",
    model: "gpt-5.6-tuna",
    status: "failed",
    createdAt: "2026-09-14T12:00:00.000Z",
    startedAt: "2026-09-14T12:00:00.000Z",
    finishedAt: "2026-09-14T12:01:00.000Z",
    computerId: null,
  },
];

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(async () => ({
    ok: true,
    json: async () => runs,
  })),
}));

afterEach(() => {
  cleanup();
});

describe("RecentRunsPanel", () => {
  it("keeps the work table on one screen with truncated assignments and single-line models", async () => {
    render(
      <MemoryRouter>
        <RecentRunsPanel limit={100} />
      </MemoryRouter>,
    );

    const assignment = await waitFor(() =>
      screen.getByRole("link", { name: longAssignment }),
    );
    expect(assignment.className).toContain("truncate");

    const table = screen.getByRole("table");
    expect(table.className).toContain("table-fixed");
    expect(table.className).toContain("w-full");

    const model = screen.getAllByText("gpt-5.6-tuna")[0];
    expect(model?.className).toContain("whitespace-nowrap");

    expect(screen.getByText("Designer")).toBeTruthy();
    expect(screen.getByText("Finished")).toBeTruthy();
    expect(screen.getByText("Needs attention")).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "Model" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "Status" })).toBeTruthy();
  });
});
