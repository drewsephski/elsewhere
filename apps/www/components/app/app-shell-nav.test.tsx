// @vitest-environment happy-dom

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { AppShellNav } from "./app-shell-nav";

describe("AppShellNav", () => {
  it("does not include Settings in management navigation", () => {
    render(
      <MemoryRouter>
        <AppShellNav showBackToChat />
      </MemoryRouter>,
    );
    expect(screen.getByRole("link", { name: "Back to chat" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Work" })).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Settings" })).toBeNull();
  });
});
