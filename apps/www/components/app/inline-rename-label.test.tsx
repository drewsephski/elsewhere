// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { InlineRenameLabel } from "./inline-rename-label";

afterEach(() => {
  cleanup();
});

describe("InlineRenameLabel", () => {
  it("opens the editor on a single click when it is the rename control", () => {
    render(
      <InlineRenameLabel
        value="Scout"
        onCommit={vi.fn()}
        ariaLabel="Rename Scout"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Rename Scout" }));
    expect(screen.getByRole("textbox", { name: "Rename Scout" })).toBeTruthy();
  });

  it("does not open the editor on a single click when nested in a row", () => {
    render(
      <InlineRenameLabel
        value="Scout"
        nested
        onCommit={vi.fn()}
        ariaLabel="Rename Scout"
      />,
    );

    fireEvent.click(screen.getByLabelText("Rename Scout"));
    expect(screen.queryByRole("textbox", { name: "Rename Scout" })).toBeNull();
    expect(screen.getByText("Scout")).toBeTruthy();
  });

  it("opens the editor on double-click when nested in a row", () => {
    render(
      <InlineRenameLabel
        value="Scout"
        nested
        onCommit={vi.fn()}
        ariaLabel="Rename Scout"
      />,
    );

    fireEvent.doubleClick(screen.getByLabelText("Rename Scout"));
    expect(screen.getByRole("textbox", { name: "Rename Scout" })).toBeTruthy();
  });
});
