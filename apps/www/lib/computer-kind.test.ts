import { describe, expect, it } from "vitest";
import { computerProviderLabel, isLocalMacProvider } from "./computer-kind";

describe("computerProviderLabel", () => {
  it("labels local Mac computers without changing Sprite copy", () => {
    expect(isLocalMacProvider("local_mac")).toBe(true);
    expect(computerProviderLabel("local_mac")).toBe("This Mac");
    expect(isLocalMacProvider("fly_sprite")).toBe(false);
    expect(computerProviderLabel("fly_sprite")).toBe("Cloud");
  });
});
