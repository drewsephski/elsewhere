// @vitest-environment happy-dom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ChatResultCards } from "./chat-result-cards";

const fetchMock = vi.fn();

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: (...args: unknown[]) => fetchMock(...args),
}));

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
});

describe("ChatResultCards", () => {
  it("keeps assignment summaries out of chat", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({
        items: [
          {
            id: "res_summary",
            name: "summary.md",
            kind: "summary",
            size: 2048,
          },
        ],
      }),
    });

    const { container } = render(<ChatResultCards runId="run_1" />);
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(container.textContent).toBe("");
    expect(screen.queryByText("Assignment summary")).toBeNull();
  });

  it("still lists file artifacts produced by the run", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({
        items: [
          {
            id: "res_summary",
            name: "summary.md",
            kind: "summary",
            size: 2048,
          },
          {
            id: "res_file",
            name: "report.md",
            kind: "file",
            size: 4096,
          },
        ],
      }),
    });

    render(<ChatResultCards runId="run_1" />);
    expect(await screen.findByText("report.md")).toBeTruthy();
    expect(screen.queryByText("Assignment summary")).toBeNull();
    expect(screen.queryByText("summary.md")).toBeNull();
  });
});
