// @vitest-environment happy-dom

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { IntegrationCard } from "./integration-card";

describe("IntegrationCard", () => {
  it("renders a compact card with name, status, and action", () => {
    render(
      <IntegrationCard
        name="GitHub"
        description="Read repositories."
        logo={<span data-testid="logo" />}
        statusLabel="Not connected"
        statusTone="neutral"
        actions={<button type="button">Connect GitHub</button>}
      />,
    );

    expect(screen.getByRole("heading", { name: "GitHub" })).toBeTruthy();
    expect(screen.getByText("Not connected")).toBeTruthy();
    expect(screen.getByTestId("logo")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Connect GitHub" })).toBeTruthy();
  });
});
