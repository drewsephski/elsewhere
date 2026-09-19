// @vitest-environment happy-dom

import { MacOSDock } from "@/components/ui/mac-os-dock";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const apps = [
  { id: "mail", name: "Mail", icon: "/app-icons/mail.svg" },
  { id: "calendar", name: "Calendar", icon: "/app-icons/calendar.svg" },
];

describe("MacOSDock", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders supplied apps", () => {
    render(<MacOSDock apps={apps} onAppClick={() => undefined} />);
    expect(screen.getByRole("toolbar", { name: "Application dock" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Mail" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Calendar" })).toBeTruthy();
  });

  it("calls onAppClick for the selected app", () => {
    const onAppClick = vi.fn();
    render(<MacOSDock apps={apps} onAppClick={onAppClick} />);
    fireEvent.click(screen.getByRole("button", { name: "Calendar" }));
    expect(onAppClick).toHaveBeenCalledWith("calendar");
  });

  it("marks open apps as pressed with an indicator", () => {
    render(<MacOSDock apps={apps} onAppClick={() => undefined} openApps={["mail"]} />);
    expect(screen.getByRole("button", { name: "Mail" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "Calendar" }).getAttribute("aria-pressed")).toBe(
      "false",
    );
  });

  it("activates apps from the keyboard", () => {
    const onAppClick = vi.fn();
    render(<MacOSDock apps={apps} onAppClick={onAppClick} />);
    fireEvent.keyDown(screen.getByRole("button", { name: "Mail" }), { key: "Enter" });
    expect(onAppClick).toHaveBeenCalledWith("mail");
  });

  it("sizes the rest tray to the icons instead of magnified hover height", () => {
    render(<MacOSDock apps={apps} onAppClick={() => undefined} size="mini" />);
    const dock = screen.getByRole("toolbar", { name: "Application dock" });
    const height = Number.parseFloat(dock.style.height);
    expect(height).toBeGreaterThan(24);
    expect(height).toBeLessThan(40);
  });
});
