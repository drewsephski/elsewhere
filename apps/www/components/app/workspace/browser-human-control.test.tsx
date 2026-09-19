// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserHumanControlBar } from "./browser-human-control";

describe("BrowserHumanControlBar", () => {
  afterEach(() => {
    cleanup();
  });

  it("hides the type field until control is taken", () => {
    render(
      <div className="relative">
        <BrowserHumanControlBar
          enabled
          humanActive={false}
          loading={false}
          error={null}
          onTakeControl={() => undefined}
          onReturnControl={() => undefined}
          onTypeText={() => undefined}
          onPressKey={() => undefined}
        />
      </div>,
    );
    expect(screen.getByRole("button", { name: "Take control" })).toBeTruthy();
    expect(screen.queryByLabelText("Type into the focused browser field")).toBeNull();
    expect(screen.queryByRole("button", { name: "Enter" })).toBeNull();
  });

  it("sends typed text and Enter from the keyboard instead of extra controls", async () => {
    const onTypeText = vi.fn();
    const onPressKey = vi.fn();
    render(
      <div className="relative">
        <BrowserHumanControlBar
          enabled
          humanActive
          loading={false}
          error={null}
          onTakeControl={() => undefined}
          onReturnControl={() => undefined}
          onTypeText={onTypeText}
          onPressKey={onPressKey}
        />
      </div>,
    );
    expect(screen.getByRole("button", { name: "Return to bot" })).toBeTruthy();
    expect(screen.queryByText("You have control")).toBeNull();
    expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
    const field = screen.getByLabelText("Type into the focused browser field");
    fireEvent.input(field, { target: { value: "hello" } });
    fireEvent.submit(field.closest("form")!);
    await waitFor(() => {
      expect(onTypeText).toHaveBeenCalledWith("hello");
      expect(onPressKey).toHaveBeenCalledWith("Enter");
    });
  });

  it("maps Tab and empty Backspace to browser keys", async () => {
    const onPressKey = vi.fn();
    render(
      <div className="relative">
        <BrowserHumanControlBar
          enabled
          humanActive
          loading={false}
          error={null}
          onTakeControl={() => undefined}
          onReturnControl={() => undefined}
          onTypeText={() => undefined}
          onPressKey={onPressKey}
        />
      </div>,
    );
    const field = screen.getByLabelText("Type into the focused browser field");
    fireEvent.keyDown(field, { key: "Tab" });
    fireEvent.keyDown(field, { key: "Backspace" });
    fireEvent.keyDown(field, { key: "Escape" });
    await waitFor(() => {
      expect(onPressKey).toHaveBeenCalledWith("Tab");
      expect(onPressKey).toHaveBeenCalledWith("Backspace");
      expect(onPressKey).toHaveBeenCalledWith("Escape");
    });
  });
});
