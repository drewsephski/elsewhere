import { describe, expect, it } from "vitest";
import {
  clampUnitRatio,
  containedImagePointerRatio,
  normalizeWheelDelta,
} from "./browser-preview-utils";

describe("containedImagePointerRatio", () => {
  it("maps clicks on a letterboxed image to the content box", () => {
    const ratio = containedImagePointerRatio(
      60,
      45,
      { left: 0, top: 0, width: 200, height: 100 },
      100,
      100,
    );
    expect(ratio).toEqual({ x: 0.1, y: 0.45 });
  });

  it("ignores clicks in the letterbox", () => {
    expect(
      containedImagePointerRatio(10, 50, { left: 0, top: 0, width: 200, height: 100 }, 100, 100),
    ).toBeNull();
  });

  it("falls back to the container when natural size is unknown", () => {
    expect(
      containedImagePointerRatio(25, 75, { left: 0, top: 0, width: 100, height: 100 }, 0, 0),
    ).toEqual({ x: 0.25, y: 0.75 });
  });
});

describe("normalizeWheelDelta", () => {
  it("clamps pixel deltas", () => {
    expect(normalizeWheelDelta(0, 12_000, 0)).toEqual({ x: 0, y: 2400 });
  });

  it("converts line-mode deltas to pixels", () => {
    expect(normalizeWheelDelta(0, 3, 1)).toEqual({ x: 0, y: 48 });
  });
});

describe("clampUnitRatio", () => {
  it("keeps values in 0..1", () => {
    expect(clampUnitRatio(-0.2)).toBe(0);
    expect(clampUnitRatio(1.4)).toBe(1);
    expect(clampUnitRatio(0.3)).toBe(0.3);
  });
});
