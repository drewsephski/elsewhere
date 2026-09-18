// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { BotCreatureAvatar } from "./bot-creature-avatar";

afterEach(() => {
  cleanup();
});

describe("BotCreatureAvatar", () => {
  it("bobs the job-bot portrait when animated", () => {
    render(<BotCreatureAvatar name="Engineer" avatarId="teal-wisp" animated />);

    expect(screen.getByRole("img", { name: "Engineer avatar" })).toBeTruthy();
    const img = document.querySelector("img");
    expect(img?.className).toContain("bot-bob");
    expect(img?.getAttribute("src")).toContain("engineer");
  });
});
