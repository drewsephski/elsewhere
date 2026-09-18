// @vitest-environment happy-dom

import type { RunDetail } from "@/lib/api-types";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkDetail } from "./work-detail";

const assignment =
  "What test color did I tell you? Do not read the file; answer only from our conversation.";

const detail: RunDetail = {
  runId: "run_1",
  requestId: "req_1",
  botId: "bot_1",
  conversationId: "conv_1",
  status: "failed",
  model: "gpt-5.6-tuna",
  computerId: "comp_1",
  startedAt: "2026-09-18T12:00:00.000Z",
  finishedAt: "2026-09-18T12:01:00.000Z",
  task: assignment,
  stepCount: 4,
  errorCode: "failed",
  assistantResult: null,
};

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(async (path: string) => {
    if (String(path).endsWith("/delegations")) {
      return { ok: true, json: async () => [] };
    }
    if (String(path).includes("/human-intervention")) {
      return { ok: true, json: async () => ({ pending: null }) };
    }
    if (String(path).endsWith("/results")) {
      return {
        ok: true,
        json: async () => ({ items: [], collecting: false, note: null }),
      };
    }
    return { ok: true, json: async () => detail };
  }),
  cloudHostEventStream: vi.fn(async (_path: string, options: {
    onEvent: (event: { id?: string; event: string; data: string }) => void;
  }) => {
    options.onEvent({
      id: "evt_1",
      event: "queued",
      data: JSON.stringify({ status: "queued" }),
    });
    options.onEvent({
      id: "evt_2",
      event: "terminal",
      data: JSON.stringify({ status: "failed" }),
    });
  }),
}));

afterEach(() => {
  cleanup();
});

describe("WorkDetail layout", () => {
  it("presents the assignment as the page heading with aligned actions", async () => {
    render(
      <MemoryRouter>
        <WorkDetail runId="run_1" />
      </MemoryRouter>,
    );

    const heading = await waitFor(() => screen.getByRole("heading", { name: assignment }));
    expect(heading.tagName).toBe("H1");
    expect(screen.getByRole("link", { name: "All work" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Open chat" })).toBeTruthy();
    expect(screen.getAllByText("Needs attention").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Archive" })).toBeTruthy();
    expect(screen.queryByText("Your message")).toBeNull();
    expect(screen.queryByText("Back to your bot")).toBeNull();
  });

  it("does not give empty files or an idle browser equal weight", async () => {
    render(
      <MemoryRouter>
        <WorkDetail runId="run_1" />
      </MemoryRouter>,
    );

    await waitFor(() => screen.getByRole("heading", { name: assignment }));
    expect(screen.getByText("Could not finish")).toBeTruthy();
    expect(screen.queryByText("No saved results yet")).toBeNull();
    expect(screen.queryByText("Send a message to watch the screen here.")).toBeNull();
    expect(screen.queryByRole("columnheader", { name: "File" })).toBeNull();
  });

  it("lists each progress step once", async () => {
    render(
      <MemoryRouter>
        <WorkDetail runId="run_1" />
      </MemoryRouter>,
    );

    const queued = await waitFor(() =>
      screen.getAllByText("Work saved. Waiting for an available computer."),
    );
    expect(queued).toHaveLength(1);
    expect(screen.getAllByText("Needs attention")).toHaveLength(2);
  });
});
