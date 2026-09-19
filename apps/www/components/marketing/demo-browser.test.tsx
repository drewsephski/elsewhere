// @vitest-environment happy-dom

import { DemoBrowser } from "@/components/marketing/demo-browser";
import { DEMO_BOTS } from "@/lib/marketing/landing";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";
import type { ReactElement } from "react";

const writer = DEMO_BOTS.find((bot) => bot.id === "writer");
const cos = DEMO_BOTS.find((bot) => bot.id === "cos");

function dock() {
  return screen.getByRole("toolbar", { name: "Application dock" });
}

function renderDemo(ui: ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>);
}

describe("DemoBrowser app dock", () => {
  afterEach(() => {
    cleanup();
  });

  it("opens Mail from the dock and updates the URL and title", () => {
    if (!cos) throw new Error("missing chief of staff demo");
    renderDemo(<DemoBrowser computer={cos.computer} controlHref="/app" />);
    fireEvent.click(within(dock()).getByRole("button", { name: "Mail" }));
    expect(screen.getByLabelText(/Inbox — mail.google.com/)).toBeTruthy();
    expect(within(dock()).getByRole("button", { name: "Mail" }).getAttribute("aria-pressed")).toBe(
      "true",
    );
    expect(screen.getByText("Nora Chen")).toBeTruthy();
  });

  it("switches to Calendar and follows the active dock indicator", () => {
    if (!writer) throw new Error("missing writer demo");
    renderDemo(<DemoBrowser computer={writer.computer} controlHref="/app" />);
    expect(within(dock()).getByRole("button", { name: "Mail" }).getAttribute("aria-pressed")).toBe(
      "true",
    );
    fireEvent.click(within(dock()).getByRole("button", { name: "Calendar" }));
    expect(screen.getByLabelText(/Week — calendar.google.com/)).toBeTruthy();
    expect(within(dock()).getByRole("button", { name: "Calendar" }).getAttribute("aria-pressed")).toBe(
      "true",
    );
    expect(within(dock()).getByRole("button", { name: "Mail" }).getAttribute("aria-pressed")).toBe(
      "false",
    );
    expect(screen.getByText("Helix Loft offsite")).toBeTruthy();
  });

  it("returns to a new-tab state when the active app is clicked again", () => {
    if (!writer) throw new Error("missing writer demo");
    renderDemo(<DemoBrowser computer={writer.computer} controlHref="/app" />);
    fireEvent.click(within(dock()).getByRole("button", { name: "Mail" }));
    expect(screen.getByLabelText(/New Tab — chrome:\/\/newtab/)).toBeTruthy();
    expect(within(dock()).getByRole("button", { name: "Browser" }).getAttribute("aria-pressed")).toBe(
      "true",
    );
    expect(within(dock()).getByRole("button", { name: "Mail" }).getAttribute("aria-pressed")).toBe(
      "false",
    );
    fireEvent.click(within(dock()).getByRole("button", { name: "Mail" }));
    expect(screen.getByLabelText(/Inbox — mail.google.com/)).toBeTruthy();
  });

  it("resets to the selected bot's computer when the demo remounts", () => {
    if (!writer || !cos) throw new Error("missing demo bots");
    const { rerender } = renderDemo(
      <DemoBrowser key="writer" computer={writer.computer} controlHref="/app" />,
    );
    fireEvent.click(within(dock()).getByRole("button", { name: "GitHub" }));
    expect(screen.getByLabelText(/Issues — github.com/)).toBeTruthy();
    expect(screen.getByText("Login race on Safari")).toBeTruthy();
    rerender(
      <MemoryRouter>
        <DemoBrowser key="cos" computer={cos.computer} controlHref="/app" />
      </MemoryRouter>,
    );
    expect(screen.getByLabelText(/Week — calendar.google.com/)).toBeTruthy();
    expect(within(dock()).getByRole("button", { name: "Calendar" }).getAttribute("aria-pressed")).toBe(
      "true",
    );
  });
});
