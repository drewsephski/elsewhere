// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceAppLayout } from "./workspace-app-layout";

vi.mock("./workspace-shell", () => ({
  WorkspaceShell: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="workspace-shell">{children}</div>
  ),
}));

vi.mock("./legacy-app-chrome", () => ({
  LegacyAppChrome: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="legacy-app-chrome">{children}</div>
  ),
}));

function renderAt(path: string, routePath: string) {
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route
          path={routePath}
          element={
            <WorkspaceAppLayout userEmail="user@example.com">
              <span data-testid="route-outlet">outlet</span>
            </WorkspaceAppLayout>
          }
        />
      </Routes>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
});

describe("WorkspaceAppLayout", () => {
  it("mounts WorkspaceShell for chat workspace routes", () => {
    renderAt("/app/bots/bot_1", "/app/bots/:id");
    expect(screen.getByTestId("workspace-shell")).toBeTruthy();
    expect(screen.getByTestId("route-outlet")).toBeTruthy();
    expect(screen.queryByTestId("legacy-app-chrome")).toBeNull();
  });

  it("mounts LegacyAppChrome for management routes", () => {
    renderAt("/app/computers", "/app/computers");
    expect(screen.getByTestId("legacy-app-chrome")).toBeTruthy();
    expect(screen.queryByTestId("workspace-shell")).toBeNull();
  });
});
