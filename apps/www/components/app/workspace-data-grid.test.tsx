// @vitest-environment happy-dom

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceDataGridTabs } from "./workspace-data-grid";

describe("WorkspaceDataGridTabs", () => {
  it("changes the active work mode without using static border tabs", () => {
    const onChange = vi.fn();
    render(
      <WorkspaceDataGridTabs
        tabs={[
          { value: "active", label: "Active" },
          { value: "archived", label: "Archived" },
        ]}
        active="active"
        onChange={onChange}
        hint="Hide finished items you no longer need."
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "Archived" }));
    expect(onChange).toHaveBeenCalledWith("archived");
    expect(screen.getByText("Hide finished items you no longer need.")).toBeTruthy();
  });
});
