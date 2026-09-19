// @vitest-environment happy-dom

import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import CloudShell from "./cloud-shell";

vi.mock("@/lib/auth-client", () => ({
  authClient: {
    getSession: vi.fn(async () => ({
      data: { user: { email: "user@example.com" } },
    })),
  },
}));

vi.mock("@/components/app/workspace/workspace-authenticated-frame", () => ({
  WorkspaceAuthenticatedFrame: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="workspace-authenticated-frame">{children}</div>
  ),
}));

vi.mock("@/components/app/desktop-shell-bootstrap", () => ({
  DesktopShellBootstrap: () => null,
}));

vi.mock("@/app/sign-in/sign-in-view", () => ({
  SignInView: () => <div>sign-in</div>,
}));

vi.mock("@/app/pair/mac/pair-mac-view", () => ({
  PairMacView: () => <div>pair-mac</div>,
}));

vi.mock("./mac-companion-connect", () => ({
  MacCompanionConnect: () => <div>mac-companion-connect</div>,
}));

vi.mock("@/app/app/computers/page", () => ({
  default: ({ headerAction }: { headerAction?: React.ReactNode }) => (
    <div>
      computers
      {headerAction}
    </div>
  ),
}));
vi.mock("@/app/app/routines/page", () => ({ default: () => <div>routines</div> }));
vi.mock("@/app/app/approvals/page", () => ({ default: () => <div>approvals</div> }));
vi.mock("@/app/app/work/page", () => ({ default: () => <div>work</div> }));
vi.mock("@/app/app/results/page", () => ({ default: () => <div>results</div> }));
vi.mock("./routes/work-detail-page", () => ({
  WorkDetailPage: () => <div>work-detail</div>,
}));

describe("CloudShell", () => {
  it("wraps authenticated /app/bots routes in WorkspaceAuthenticatedFrame (shared web workspace)", async () => {
    render(
      <MemoryRouter initialEntries={["/app/bots/bot_1"]}>
        <CloudShell />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("workspace-authenticated-frame")).toBeTruthy();
    });
  });

  it("keeps /pair/mac instead of redirecting to /app", async () => {
    render(
      <MemoryRouter initialEntries={["/pair/mac?pairingId=abc&userCode=xyz"]}>
        <CloudShell />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByText("pair-mac")).toBeTruthy();
    });
  });

  it("mounts Mac Companion connect on /app/computers", async () => {
    render(
      <MemoryRouter initialEntries={["/app/computers"]}>
        <CloudShell />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByText("computers")).toBeTruthy();
      expect(screen.getByText("mac-companion-connect")).toBeTruthy();
    });
  });
});
