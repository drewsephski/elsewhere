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

vi.mock("@/components/app/workspace/workspace-app-layout", () => ({
  WorkspaceAppLayout: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="workspace-app-layout">{children}</div>
  ),
}));

vi.mock("@/components/app/product-theme-scope", () => ({
  ProductThemeScope: () => null,
}));

vi.mock("@/components/ui/sonner", () => ({
  Toaster: () => null,
}));

vi.mock("@/app/sign-in/sign-in-view", () => ({
  SignInView: () => <div>sign-in</div>,
}));

vi.mock("@/app/app/computers/page", () => ({ default: () => <div>computers</div> }));
vi.mock("@/app/app/routines/page", () => ({ default: () => <div>routines</div> }));
vi.mock("@/app/app/approvals/page", () => ({ default: () => <div>approvals</div> }));
vi.mock("@/app/app/work/page", () => ({ default: () => <div>work</div> }));
vi.mock("@/app/app/results/page", () => ({ default: () => <div>results</div> }));
vi.mock("./routes/work-detail-page", () => ({
  WorkDetailPage: () => <div>work-detail</div>,
}));

describe("CloudShell", () => {
  it("wraps authenticated /app/bots routes in WorkspaceAppLayout (shared workspace shell)", async () => {
    render(
      <MemoryRouter initialEntries={["/app/bots/bot_1"]}>
        <CloudShell />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("workspace-app-layout")).toBeTruthy();
    });
  });
});
