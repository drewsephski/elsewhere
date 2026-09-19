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
    expect(root.querySelector(".creature-ring")).toBeNull();
    expect(root.querySelector(".creature-spark")).toBeNull();
    const img = root.querySelector("img");
    expect(img?.getAttribute("src")).toContain("engineer");
  });

  it("hides working chrome when idle", () => {
    render(<BotCreatureAvatar name="Engineer" avatarId="teal-wisp" />);

    const root = screen.getByRole("img", { name: "Engineer avatar" });
    expect(root.getAttribute("data-working")).toBe("off");
    expect(root.querySelector("[data-creature-aura]")).toBeNull();
    expect(root.querySelector(".creature-spec")).toBeNull();
  });

  it("keeps a lighting sweep on md working portraits and skips it at xs", () => {
    const { rerender } = render(
      <BotCreatureAvatar name="Engineer" avatarId="teal-wisp" size="md" animated />,
    );
    const hero = screen.getByRole("img", { name: "Engineer avatar" });
    expect(hero.getAttribute("data-intensity")).toBe("hero");
    expect(hero.querySelector(".creature-spec")).toBeTruthy();
    expect(hero.querySelector(".creature-ground")).toBeTruthy();

    rerender(<BotCreatureAvatar name="Engineer" avatarId="teal-wisp" size="xs" animated />);
    const compact = screen.getByRole("img", { name: "Engineer avatar" });
    expect(compact.getAttribute("data-intensity")).toBe("compact");
    expect(compact.querySelector(".creature-spec")).toBeNull();
    expect(compact.querySelector(".creature-ground")).toBeNull();
  });
});
