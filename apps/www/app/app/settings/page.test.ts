import { describe, expect, it } from "vitest";
import SettingsPage from "./page";

describe("settings page", () => {
  it("redirects to the unified settings dialog", () => {
    expect(() => SettingsPage()).toThrow(/\/app\?settings=workspace/);
  });
});
