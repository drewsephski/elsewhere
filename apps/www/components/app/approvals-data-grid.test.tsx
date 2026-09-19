// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApprovalsDataGrid } from "./approvals-data-grid";

const fetchMock = vi.fn();

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: (...args: unknown[]) => fetchMock(...args),
}));

vi.mock("next/link", () => ({
  default: function MockLink({
    href,
    children,
    className,
  }: {
    href: string;
    children: React.ReactNode;
    className?: string;
  }) {
    return (
      <a href={href} className={className}>
        {children}
      </a>
    );
  },
}));

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
});

describe("ApprovalsDataGrid", () => {
  it("shows the human action label and Bot name instead of raw tool identifiers", async () => {
    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          approvals: [
            {
              approvalId: "appr_1",
              runId: "run_1",
              toolName: "workspace_write",
              toolKind: "mutation",
              status: "pending",
              summary: "Write /workspace/brief.md",
              botId: "bot_1",
              botName: "Scout",
              policyActionLabel: "Write files",
            },
          ],
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
    render(<ApprovalsDataGrid />);
    expect(await screen.findByText("Write /workspace/brief.md")).toBeTruthy();
    expect(screen.getByText("Write files · Scout")).toBeTruthy();
    expect(screen.queryByText("workspace_write · mutation")).toBeNull();
  });
});
