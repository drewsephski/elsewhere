// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";
import { WorkStatusCard } from "./work-status-card";

afterEach(() => {
  cleanup();
});

const run = {
  runId: "run_1",
};

function renderCard(ui: ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>);
}

describe("WorkStatusCard", () => {
  it("renders a quiet View work link instead of a status card", () => {
    renderCard(
      <WorkStatusCard run={run}>
        <span>Report.md</span>
      </WorkStatusCard>,
    );

    const link = screen.getByRole("link", { name: "View work" });
    expect(link.getAttribute("href")).toBe("/app/work/run_1");
    expect(screen.getByText("Report.md")).toBeTruthy();
    expect(screen.queryByLabelText("Work status")).toBeNull();
    expect(screen.queryByText("Waiting")).toBeNull();
  });

  it("keeps secondary actions next to the link", () => {
    renderCard(<WorkStatusCard run={run} actions={<button type="button">Archive</button>} />);

    expect(screen.getByRole("link", { name: "View work" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Archive" })).toBeTruthy();
  });
});
