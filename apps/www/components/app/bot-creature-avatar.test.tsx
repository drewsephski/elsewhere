// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { BotCreatureAvatar } from "./bot-creature-avatar";

afterEach(() => {
  cleanup();
});

describe("BotCreatureAvatar", () => {
  it("shows working chrome when animated", () => {
    render(<BotCreatureAvatar name="Engineer" avatarId="teal-wisp" animated />);

    const root = screen.getByRole("img", { name: "Engineer avatar" });
    expect(root.getAttribute("data-working")).toBe("on");
    expect(root.querySelector("[data-creature-aura]")).toBeTruthy();
    const img = root.querySelector("img");
    expect(img?.className).not.toContain("bot-bob");
    expect(img?.getAttribute("src")).toContain("engineer");
  });

  it("hides working chrome when idle", () => {
    render(<BotCreatureAvatar name="Engineer" avatarId="teal-wisp" />);

    const root = screen.getByRole("img", { name: "Engineer avatar" });
    expect(root.getAttribute("data-working")).toBe("off");
    expect(root.querySelector("[data-creature-aura]")).toBeNull();
  });

  it("puts a tablet phosphor on sm+ working portraits and skips it at xs", () => {
    const { rerender } = render(
      <BotCreatureAvatar name="Engineer" avatarId="teal-wisp" size="md" animated />,
    );
    const hero = screen.getByRole("img", { name: "Engineer avatar" });
    expect(hero.querySelector(".creature-tablet")).toBeTruthy();
    expect(hero.style.getPropertyValue("--creature-phase")).toMatch(/-?\d+(\.\d+)?s/);

    rerender(<BotCreatureAvatar name="Engineer" avatarId="teal-wisp" size="xs" animated />);
    const compact = screen.getByRole("img", { name: "Engineer avatar" });
    expect(compact.querySelector(".creature-tablet")).toBeNull();
  });
});
