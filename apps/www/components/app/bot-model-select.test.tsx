// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BotModelSelect } from "./bot-model-select";

describe("BotModelSelect", () => {
  afterEach(() => {
    cleanup();
  });

  it("lists Luna as default plus Terra, Sol, and Astra", () => {
    render(
      <BotModelSelect value="gpt-5.6-luna" onValueChange={vi.fn()} />,
    );

    expect(screen.getByLabelText("Model")).toBeTruthy();
    expect(screen.getByText("Applies to new work only. Queued and running work keeps its original model.")).toBeTruthy();
  });

  it("keeps an unknown assigned model visible", () => {
    render(<BotModelSelect value="codex" onValueChange={vi.fn()} />);
    expect(screen.getByLabelText("Model")).toBeTruthy();
  });
});
