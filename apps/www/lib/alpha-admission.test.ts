import { describe, expect, it } from "vitest";
import { acceptsAlphaInvitation } from "./alpha-admission";

describe("alpha account admission", () => {
  it("fails closed when the hosted invitation is missing or incorrect", () => {
    expect(acceptsAlphaInvitation(null, undefined, true)).toBe(false);
    expect(acceptsAlphaInvitation("anything", undefined, true)).toBe(false);
    expect(acceptsAlphaInvitation(null, "private-invitation", true)).toBe(false);
    expect(acceptsAlphaInvitation("incorrect", "private-invitation", true)).toBe(false);
    expect(acceptsAlphaInvitation("private-invitation", "private-invitation", true)).toBe(true);
  });

  it("preserves local signup, while honoring a configured gate even without the UI flag", () => {
    expect(acceptsAlphaInvitation(null, undefined, false)).toBe(true);
    expect(acceptsAlphaInvitation(null, "private-invitation", false)).toBe(false);
  });
});
