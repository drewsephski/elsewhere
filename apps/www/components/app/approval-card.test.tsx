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
});
