// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProfileFooter } from "./profile-footer";

vi.mock("@/lib/auth-client", () => ({
  authClient: {
    signOut: vi.fn(async () => undefined),
  },
}));

describe("ProfileFooter", () => {
  afterEach(() => {
    cleanup();
  });

  it("opens the account menu without throwing", () => {
    const onOpenSettings = vi.fn();
    render(
      <MemoryRouter>
        <ProfileFooter email="user@example.com" onOpenSettings={onOpenSettings} />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Account menu" }));

    expect(screen.getByText("Settings")).toBeTruthy();
    expect(screen.getByText("Sign out")).toBeTruthy();
  });
});
