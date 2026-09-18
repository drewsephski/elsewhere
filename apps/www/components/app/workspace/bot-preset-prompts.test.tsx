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

  it("shows the short label and fills the fuller prompt on click", () => {
    const onSelect = vi.fn();
    render(
      <BotPresetPrompts
        prompts={[
          {
            label: "Write a sourced brief",
            prompt: "Research this topic and write a concise sourced brief: [topic].",
          },
          {
            label: "Find the latest",
            prompt: "Find the latest developments on [topic] from the last 30 days.",
          },
        ]}
        onSelect={onSelect}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Write a sourced brief" }));
    expect(onSelect).toHaveBeenCalledWith(
      "Research this topic and write a concise sourced brief: [topic].",
    );
    expect(screen.getByRole("list", { name: "Suggested prompts" })).toBeTruthy();
  });
});
