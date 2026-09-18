// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApprovalCard, type ApprovalRequestedPayload } from "./approval-card";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(),
}));

const payload: ApprovalRequestedPayload = {
  approvalId: "appr_1",
  tool: "workspace_write",
  operationKind: "write",
  summary: "Write report.md in the project folder",
  botName: "Scout",
  connectedAppName: "Workspace",
  connectedToolName: "Write file",
  argumentSummary: { path: "report.md" },
};

afterEach(() => {
  cleanup();
});

describe("ApprovalCard", () => {
  it("shows Allow and Deny with human-readable target", () => {
    render(<ApprovalCard payload={payload} />);
    expect(screen.getByText("Allow this action?")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Allow" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Deny" })).toBeTruthy();
    expect(screen.getByText(/Target:/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Show details" })).toBeTruthy();
    expect(document.querySelector("pre")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show details" }));
    expect(document.querySelector("pre")?.textContent).toContain("report.md");
  });

  it("uses routine-specific title for routine tools", () => {
    render(
      <ApprovalCard
        payload={{
          ...payload,
          tool: "routine_create",
          summary: 'Run "Morning brief" every weekday at 08:00 (America/Chicago)',
        }}
      />,
    );
    expect(screen.getByText("Allow this routine?")).toBeTruthy();
  });

  it("renders resolved approvals with neutral copy and no waiting language", () => {
    render(<ApprovalCard payload={payload} externalStatus="approved" />);
    expect(screen.getByText(/Allowed · Write report\.md/)).toBeTruthy();
    expect(screen.queryByText(/waits until you choose/i)).toBeNull();
    expect(screen.queryByRole("button", { name: "Allow" })).toBeNull();
    const region = screen.getByRole("region", { name: /Allowed ·/ });
    expect(region.textContent?.split(payload.summary).length).toBe(2);
  });
});
