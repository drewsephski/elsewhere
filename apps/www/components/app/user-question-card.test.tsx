// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { UserQuestionCard } from "./user-question-card";
import { cloudHostFetch } from "@/lib/cloud-api";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(),
}));

const fetchMock = vi.mocked(cloudHostFetch);

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
});

describe("UserQuestionCard", () => {
  it("renders answered questions with neutral resolved copy", () => {
    render(
      <UserQuestionCard
        botName="Scout"
        question={{
          questionId: "q1",
          runId: "run_1",
          question: "Deploy where?",
          options: ["Production", "Staging"],
          selectedIndex: 1,
          status: "answered",
        }}
      />,
    );
    expect(screen.getByText(/Choice saved · Staging/)).toBeTruthy();
    expect(screen.queryByText(/needs your choice/i)).toBeNull();
    expect(screen.queryByText(/Pick one option/i)).toBeNull();
    expect(screen.queryByRole("button", { name: "Skip for now" })).toBeNull();
  });

  it("posts the selected index to the run question endpoint", async () => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ status: "answered", selectedIndex: 0 }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    render(
      <UserQuestionCard
        botName="Scout"
        question={{
          questionId: "q1",
          runId: "run_1",
          question: "Deploy where?",
          options: ["Production", "Staging"],
          status: "pending",
        }}
      />,
    );
    fireEvent.click(screen.getByRole("radio", { name: "Production" }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/v1/runs/run_1/questions/q1/answer",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({ selectedIndex: 0 }),
        }),
      );
    });
  });
});
