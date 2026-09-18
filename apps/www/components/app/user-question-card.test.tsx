// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { UserQuestionCard } from "./user-question-card";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(),
}));

afterEach(() => {
  cleanup();
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
  });
});
