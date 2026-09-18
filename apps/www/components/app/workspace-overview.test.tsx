// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkspaceOverview } from "./workspace-overview";

vi.mock("@/hooks/use-workspace-overview", () => ({
  useWorkspaceOverview: () => ({
    data: {
      runnerReady: true,
      counts: {
        working: 0,
        queued: 0,
        approvals: 0,
        finished: 0,
        results: 0,
        routines: 0,
      },
      bots: [],
    },
    error: null,
    phase: "ready",
    refresh: vi.fn(),
  }),
}));

afterEach(() => {
  cleanup();
});

describe("WorkspaceOverview empty state", () => {
  it("points first-run users at the workspace instead of computers or advanced create", () => {
    render(
      <MemoryRouter>
        <WorkspaceOverview />
      </MemoryRouter>,
    );

    expect(screen.getByText("Give your first bot a job")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Start in the workspace" }).getAttribute("href")).toBe(
      "/app",
    );
    expect(screen.queryByRole("link", { name: "Create a computer" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Create a bot" })).toBeNull();
  });
});
