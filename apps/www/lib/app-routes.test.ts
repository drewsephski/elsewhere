import { describe, expect, it } from "vitest";
import { appRoutes, appShellNavLinks } from "./app-routes";
import { parseSettingsSection, settingsDialogHref } from "./settings-sections";

describe("appShellNavLinks", () => {
  it("does not expose a standalone Settings destination", () => {
    expect(appShellNavLinks.map((link) => link.label)).not.toContain("Settings");
    expect(appShellNavLinks.map((link) => link.href)).not.toContain(appRoutes.settings);
  });

  it("does not expose Channels in management navigation", () => {
    expect(appShellNavLinks.map((link) => link.label)).not.toContain("Channels");
    expect(appShellNavLinks.map((link) => link.href)).not.toContain(appRoutes.channels);
  });
});

describe("parseSettingsSection", () => {
  it("accepts known sections and rejects unknown values", () => {
    expect(parseSettingsSection("workspace")).toBe("workspace");
    expect(parseSettingsSection("chatgpt")).toBe("chatgpt");
    expect(parseSettingsSection("nope")).toBeNull();
    expect(parseSettingsSection(null)).toBeNull();
  });

  it("builds a workspace deep link", () => {
    expect(settingsDialogHref("workspace")).toBe("/app?settings=workspace");
  });
});
