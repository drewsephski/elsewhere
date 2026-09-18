// @vitest-environment happy-dom

import type { BotSummary, Routine } from "@/lib/api-types";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cloudHostFetch } from "@/lib/cloud-api";
import { RoutinesManager } from "./routines-manager";

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: vi.fn(),
    replace: vi.fn(),
    refresh: vi.fn(),
  }),
}));

vi.mock("next/link", () => ({
  default: ({
    href,
    children,
    ...props
  }: {
    href: string;
    children: React.ReactNode;
  }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(),
}));

const fetchMock = vi.mocked(cloudHostFetch);

const bot: BotSummary = {
  id: "bot_1",
  name: "Brief bot",
  instructions: "Write briefs",
  model: "codex",
  computerId: "comp_1",
  enginePreference: "codex",
};

const routine: Routine = {
  id: "rtn_1",
  botId: "bot_1",
  name: "Morning project brief",
  instructions: "Review project files and prepare a brief.",
  intervalMinutes: 1440,
  enabled: true,
  nextRunAt: "2026-09-18T13:34:00.000Z",
  lastRunId: "run_1",
  lastError: null,
  scheduleKind: "interval",
  scheduleExpression: "1440",
  timezone: "America/Chicago",
  scheduleLabel: "Every 24 hours",
  destinationConversationId: null,
  lastSuccessAt: null,
  lastFailureAt: null,
  consecutiveFailures: 0,
  failurePolicy: "pause_after_failure",
  triggerMode: "schedule",
  webhook: { configured: false },
  recentRuns: [],
};

function mockLists(routines: Routine[] = [], bots: BotSummary[] = [bot]) {
  fetchMock.mockImplementation(async (path: string) => {
    const payload =
      path === "/v1/routines"
        ? routines
        : path === "/v1/bots"
          ? bots
          : [];
    return {
      ok: true,
      json: async () => payload,
    } as Response;
  });
}

describe("RoutinesManager", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("shows an empty state instead of a table and opens a grouped form", async () => {
    mockLists([]);
    render(<RoutinesManager />);

    await waitFor(() => {
      expect(screen.getByText("No routines yet")).toBeTruthy();
    });

    expect(screen.queryByText("Trigger")).toBeNull();
    expect(screen.queryByText("Create a routine")).toBeNull();

    fireEvent.click(screen.getAllByRole("button", { name: "Create a new routine" })[0]!);

    expect(screen.getByRole("heading", { name: "New routine" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Work" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "When" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Results" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create routine" })).toBeTruthy();
  });

  it("renders existing routines as a scannable list without the create form", async () => {
    mockLists([routine]);
    render(<RoutinesManager />);

    await waitFor(() => {
      expect(screen.getByText("Morning project brief")).toBeTruthy();
    });

    expect(screen.getByText("Active")).toBeTruthy();
    expect(screen.getByText(/Every 24 hours/)).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "New routine" })).toBeNull();
    expect(screen.getByRole("link", { name: /Morning project brief/ })).toHaveProperty(
      "href",
      expect.stringContaining("/app/routines/rtn_1"),
    );
  });
});
