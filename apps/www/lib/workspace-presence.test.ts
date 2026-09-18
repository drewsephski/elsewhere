import { describe, expect, it } from "vitest";
import { instructionPreview } from "./format";
import {
  presenceDotClass,
  presenceShortLabel,
  presenceSidebarTrailing,
  presenceTone,
} from "./workspace-types";

describe("instructionPreview", () => {
  it("returns compact truncated copy", () => {
    expect(instructionPreview("  Research, compare sources.  ")).toBe(
      "Research, compare sources.",
    );
    expect(instructionPreview("a".repeat(200)).endsWith("…")).toBe(true);
    expect(instructionPreview("   ")).toBe("");
  });
});

describe("bot presence labels", () => {
  it("maps backend presence to short labels and tones", () => {
    expect(presenceShortLabel("working")).toBe("Working");
    expect(presenceShortLabel("ready")).toBe("Ready");
    expect(presenceShortLabel("needs_attention")).toBe("Needs you");
    expect(presenceShortLabel("waiting_approval")).toBe("Approve");
    expect(presenceShortLabel("unknown")).toBeNull();
    expect(presenceTone("working")).toBe("info");
    expect(presenceTone("ready")).toBe("success");
    expect(presenceTone("waiting_approval")).toBe("warning");
    expect(presenceDotClass("working")).toContain("info");
  });

  it("prioritizes attention and active labels in the sidebar", () => {
    expect(presenceSidebarTrailing("waiting_approval").emphasis).toBe("attention");
    expect(presenceSidebarTrailing("working").statusLabel).toBe("Working");
    expect(presenceSidebarTrailing("ready").statusLabel).toBeNull();
  });
});
