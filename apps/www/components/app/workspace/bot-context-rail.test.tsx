// @vitest-environment happy-dom

import type { BotSummary } from "@/lib/api-types";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
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
    expect(screen.getByText("Routines")).toBeTruthy();
    expect(screen.getByText("Files")).toBeTruthy();
    expect(screen.getByText("Memory")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Delete bot" })).toBeNull();
    expect(screen.queryByLabelText("Name")).toBeNull();
    expect(screen.queryByText("Permissions")).toBeNull();
  });
});
