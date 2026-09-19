// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserControlSwitcher, BrowserRemoteSurface } from "./browser-human-control";

describe("BrowserControlSwitcher", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows watching state until control is taken", () => {
    render(
      <BrowserControlSwitcher
        enabled
        humanActive={false}
        loading={false}
        onTakeControl={() => undefined}
        onReturnControl={() => undefined}
      />,
    );
    expect(screen.getByText("Bot is driving")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Take control" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Return to bot" })).toBeNull();
  });

  it("shows return to bot after takeover", () => {
    render(
      <BrowserControlSwitcher
        enabled
        humanActive
        loading={false}
        onTakeControl={() => undefined}
        onReturnControl={() => undefined}
      />,
    );
    expect(screen.getByText("You're in control")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Return to bot" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Take control" })).toBeNull();
  });
});

describe("BrowserRemoteSurface", () => {
  afterEach(() => {
    cleanup();
  });

  it("takes control on the first click while watching", async () => {
    const onTakeControl = vi.fn();
    render(
      <div className="relative h-40 w-40">
        <BrowserRemoteSurface
          enabled
          humanActive={false}
          busy={false}
          onTakeControl={onTakeControl}
          onPreviewClick={() => undefined}
          onTypeText={() => undefined}
          onPressKey={() => undefined}
          onScroll={() => undefined}
        />
      </div>,
    );
    fireEvent.pointerDown(screen.getByRole("application"), { button: 0, clientX: 20, clientY: 20 });
    await waitFor(() => {
      expect(onTakeControl).toHaveBeenCalledTimes(1);
    });
  });

  it("sends a click at the pointer location once in control", async () => {
    const onPreviewClick = vi.fn();
    render(
      <div className="relative h-40 w-40">
        <BrowserRemoteSurface
          enabled
          humanActive
          busy={false}
          onTakeControl={() => undefined}
          onPreviewClick={onPreviewClick}
          onTypeText={() => undefined}
          onPressKey={() => undefined}
          onScroll={() => undefined}
        />
      </div>,
    );
    const surface = screen.getByRole("application");
    vi.spyOn(surface, "getBoundingClientRect").mockReturnValue({
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: 100,
      bottom: 100,
      width: 100,
      height: 100,
      toJSON() {
        return {};
      },
    });
    fireEvent.pointerDown(surface, { button: 0, clientX: 25, clientY: 50 });
    await waitFor(() => {
      expect(onPreviewClick).toHaveBeenCalledWith(0.25, 0.5);
    });
  });

  it("types into the live view instead of a separate field", async () => {
    const onTypeText = vi.fn();
    const onPressKey = vi.fn();
    render(
      <div className="relative h-40 w-40">
        <BrowserRemoteSurface
          enabled
          humanActive
          busy={false}
          onTakeControl={() => undefined}
          onPreviewClick={() => undefined}
          onTypeText={onTypeText}
          onPressKey={onPressKey}
          onScroll={() => undefined}
        />
      </div>,
    );
    expect(screen.queryByLabelText("Type into the focused browser field")).toBeNull();
    const surface = screen.getByRole("application");
    fireEvent.keyDown(surface, { key: "h" });
    fireEvent.keyDown(surface, { key: "i" });
    await waitFor(() => {
      expect(onTypeText).toHaveBeenCalledWith("hi");
    });
    fireEvent.keyDown(surface, { key: "Enter" });
    await waitFor(() => {
      expect(onPressKey).toHaveBeenCalledWith("Enter");
    });
  });

  it("scrolls the live view with the wheel", async () => {
    vi.useFakeTimers();
    const onScroll = vi.fn();
    try {
      render(
        <div className="relative h-40 w-40">
          <BrowserRemoteSurface
            enabled
            humanActive
            busy={false}
            onTakeControl={() => undefined}
            onPreviewClick={() => undefined}
            onTypeText={() => undefined}
            onPressKey={() => undefined}
            onScroll={onScroll}
          />
        </div>,
      );
      const surface = screen.getByRole("application");
      vi.spyOn(surface, "getBoundingClientRect").mockReturnValue({
        x: 0,
        y: 0,
        left: 0,
        top: 0,
        right: 100,
        bottom: 100,
        width: 100,
        height: 100,
        toJSON() {
          return {};
        },
      });
      surface.dispatchEvent(
        new WheelEvent("wheel", {
          clientX: 40,
          clientY: 40,
          deltaY: 120,
          deltaX: 0,
          deltaMode: 0,
          bubbles: true,
          cancelable: true,
        }),
      );
      await vi.advanceTimersByTimeAsync(60);
      expect(onScroll).toHaveBeenCalledTimes(1);
      expect(onScroll.mock.calls[0]?.[2]).toBe(0);
      expect(onScroll.mock.calls[0]?.[3]).toBe(120);
    } finally {
      vi.useRealTimers();
    }
  });
});
