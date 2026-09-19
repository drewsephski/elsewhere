// @vitest-environment happy-dom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BotLibraryDialog } from "./bot-library-dialog";
import type { LibraryOverlay } from "@/lib/workspace-chrome";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(async () => ({
    ok: true,
    json: async () => [],
  })),
}));

const closed: LibraryOverlay = { status: "closed" };

const routinesOpen: LibraryOverlay = {
  status: "open",
  library: "routines",
  botId: "bot_1",
};

const skillsOpen: LibraryOverlay = {
  status: "open",
  library: "skills",
  botId: "bot_1",
};

const weekdayBrief = /Every weekday at 8 AM, send my morning brief/;

function renderDialog(overlay: LibraryOverlay) {
  return render(
    <MemoryRouter>
      <BotLibraryDialog overlay={overlay} onClose={vi.fn()} />
    </MemoryRouter>,
  );
}

describe("BotLibraryDialog", () => {
  afterEach(() => {
    cleanup();
  });

  it("does not show Routines or Skills titles while closed", () => {
    renderDialog(closed);
    expect(screen.queryByRole("heading", { name: "Routines" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Skills" })).toBeNull();
    expect(screen.queryByText(weekdayBrief)).toBeNull();
  });

  it("shows Routines and weekday-brief empty copy while open", async () => {
    renderDialog(routinesOpen);
    expect(screen.getByRole("heading", { name: "Routines" }).textContent).toBe("Routines");
    await waitFor(() => {
      expect(screen.getByText(weekdayBrief).textContent).toBe(
        "No recurring work yet. Tell this Bot in chat what to run on a schedule — for example, “Every weekday at 8 AM, send my morning brief.”",
      );
    });
  });

  it("replaces routines copy with Skills when the overlay library changes", async () => {
    const view = renderDialog(routinesOpen);
    await waitFor(() => {
      expect(screen.getByText(weekdayBrief).textContent).toBe(
        "No recurring work yet. Tell this Bot in chat what to run on a schedule — for example, “Every weekday at 8 AM, send my morning brief.”",
      );
    });

    view.rerender(
      <MemoryRouter>
        <BotLibraryDialog overlay={skillsOpen} onClose={vi.fn()} />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Skills" }).textContent).toBe("Skills");
    expect(screen.queryByRole("heading", { name: "Routines" })).toBeNull();
    expect(screen.queryByText(weekdayBrief)).toBeNull();
    await waitFor(() => {
      expect(
        screen.getByText(
          "No skills attached yet. Save successful work from chat or manage skills in settings.",
        ).textContent,
      ).toBe("No skills attached yet. Save successful work from chat or manage skills in settings.");
    });
  });
});
