// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RunFailureCard } from "./run-failure-card";

afterEach(() => {
  cleanup();
});

describe("RunFailureCard", () => {
  it("prefills via onRetryMessage without submitting", () => {
    const onRetryMessage = vi.fn();
    render(
      <RunFailureCard
        runId="run_1"
        status="failed"
        errorCode="codex_not_authenticated"
        onRetryMessage={onRetryMessage}
      />,
    );
    expect(screen.getByText("Connect ChatGPT")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetryMessage).toHaveBeenCalledTimes(1);
  });
});
