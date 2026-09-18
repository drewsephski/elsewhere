// @vitest-environment happy-dom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ResultsPanel } from "./results-panel";

const fetchMock = vi.fn();

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: (...args: unknown[]) => fetchMock(...args),
}));

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
});

describe("ResultsPanel compact", () => {
  it("collapses when a work run has no files", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ items: [], collecting: false, note: null }),
    });

    const { container } = render(<ResultsPanel runId="run_1" variant="compact" />);
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(container.textContent).toBe("");
    expect(screen.queryByText("No saved results yet")).toBeNull();
    expect(screen.queryByRole("columnheader", { name: "File" })).toBeNull();
  });

  it("lists saved files without the full results table", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({
        items: [
          {
            id: "res_1",
            runId: "run_1",
            name: "notes.md",
            kind: "file",
            size: 2048,
            botName: "Scout",
            task: "Write notes",
            createdAt: "2026-09-18T12:00:00.000Z",
          },
        ],
        collecting: false,
        note: null,
      }),
    });

    render(<ResultsPanel runId="run_1" variant="compact" />);
    expect(await screen.findByRole("heading", { name: "Files" })).toBeTruthy();
    expect(screen.getByText("notes.md")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Download" })).toBeTruthy();
    expect(screen.queryByRole("columnheader", { name: "Bot" })).toBeNull();
  });
});
