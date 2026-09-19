import { demoComputerForDockApp, demoDockAppIdForPage } from "@/lib/marketing/landing";
import { describe, expect, it } from "vitest";

describe("demo dock catalog", () => {
  it("maps page kinds onto the shared dock apps", () => {
    expect(demoDockAppIdForPage("mail")).toBe("mail");
    expect(demoDockAppIdForPage("sheet")).toBe("docs");
    expect(demoDockAppIdForPage("accounts")).toBeNull();
  });

  it("reuses the existing demo computers instead of duplicating pages", () => {
    expect(demoComputerForDockApp("mail").page.kind).toBe("mail");
    expect(demoComputerForDockApp("calendar").page.kind).toBe("calendar");
    expect(demoComputerForDockApp("github").page.kind).toBe("github");
    expect(demoComputerForDockApp("docs").page.kind).toBe("sheet");
    expect(demoComputerForDockApp("figma").page.kind).toBe("figma");
  });
});
