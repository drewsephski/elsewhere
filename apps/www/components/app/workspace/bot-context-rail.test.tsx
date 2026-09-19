// @vitest-environment happy-dom

import type { BotSummary } from "@/lib/api-types";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BotContextRail } from "./bot-context-rail";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(async () => ({
    ok: true,
    json: async () => [],
  })),
}));

const bot: BotSummary = {
  id: "bot_1",
  name: "Researcher",
  instructions: "Research, compare sources, and deliver a concise result.",
  model: "codex",
  computerId: "comp_1",
  enginePreference: "codex",
};

describe("BotContextRail", () => {
  afterEach(() => {
    cleanup();
  });
  it("keeps the gear and live context, without embedding Settings forms", () => {
    const onOpenSettings = vi.fn();
    render(
      <MemoryRouter>
        <BotContextRail
          bot={bot}
          activeRun={null}
          onBotSaved={vi.fn()}
          onOpenSettings={onOpenSettings}
        />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Bot settings" }));
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Collapse sidebar" })).toBeNull();
    expect(screen.getByRole("heading", { name: "Computer" }).textContent).toBe("Computer");
    expect(screen.getByRole("button", { name: "Files" }).textContent).toContain("Files");
    expect(screen.getByRole("button", { name: "Memory" }).textContent).toContain("Memory");
    expect(screen.queryByRole("button", { name: "Routines" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Skills" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Delete bot" })).toBeNull();
    expect(screen.queryByLabelText("Name")).toBeNull();
    expect(screen.queryByText("Permissions")).toBeNull();
  });

  it("places the collapse control in the sidebar header beside settings", () => {
    const onCollapseRail = vi.fn();
    render(
      <MemoryRouter>
        <BotContextRail
          bot={bot}
          activeRun={null}
          onBotSaved={vi.fn()}
          onOpenSettings={vi.fn()}
          onCollapseRail={onCollapseRail}
        />
      </MemoryRouter>,
    );

    const collapse = screen.getByRole("button", { name: "Collapse sidebar" });
    const settings = screen.getByRole("button", { name: "Bot settings" });
    expect(collapse.compareDocumentPosition(settings) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    fireEvent.click(collapse);
    expect(onCollapseRail).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Hide details" })).toBeNull();
  });
});
