// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BotPresetPrompts } from "./bot-preset-prompts";

describe("BotPresetPrompts", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders nothing without prompts", () => {
    render(<BotPresetPrompts prompts={[]} onSelect={vi.fn()} />);
    expect(screen.queryByRole("list", { name: "Suggested prompts" })).toBeNull();
  });

  it("lets a click choose a prompt", () => {
    const onSelect = vi.fn();
    render(
      <BotPresetPrompts
        prompts={["Write a sourced brief", "Find the latest"]}
        onSelect={onSelect}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Write a sourced brief" }));
    expect(onSelect).toHaveBeenCalledWith("Write a sourced brief");
    expect(screen.getByRole("list", { name: "Suggested prompts" })).toBeTruthy();
  });
});
