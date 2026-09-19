import { dockAppIdForUrl, isNeutralBrowserUrl } from "@/lib/browser-dock";
import { describe, expect, it } from "vitest";

describe("browser dock URL matching", () => {
  it("selects the dock app from the live hostname", () => {
    expect(dockAppIdForUrl("https://mail.google.com/mail/u/0/#inbox")).toBe("mail");
    expect(dockAppIdForUrl("https://calendar.google.com")).toBe("calendar");
    expect(dockAppIdForUrl("https://github.com/drewsephski/elsewhere")).toBe("github");
    expect(dockAppIdForUrl("https://docs.google.com/spreadsheets/d/1")).toBe("docs");
    expect(dockAppIdForUrl("https://www.figma.com/file/abc")).toBe("figma");
    expect(dockAppIdForUrl("https://www.google.com/search?q=elsewhere")).toBe("browser");
  });

  it("does not treat Google app hosts as the generic browser", () => {
    expect(dockAppIdForUrl("https://mail.google.com")).not.toBe("browser");
    expect(dockAppIdForUrl("https://docs.google.com")).not.toBe("browser");
  });

  it("clears the active app on a blank browser page", () => {
    expect(isNeutralBrowserUrl("about:blank")).toBe(true);
    expect(dockAppIdForUrl("about:blank")).toBeNull();
    expect(dockAppIdForUrl(null)).toBeNull();
  });
});
