import { describe, expect, test } from "vitest";
import {
  clampPipPosition,
  defaultPipPosition,
  PIP_MARGIN,
  PIP_MIN_HEIGHT,
  PIP_WIDTH,
} from "./browser-preview-pip";

describe("browser preview pip geometry", () => {
  test("defaults to the upper-right of the chat pane", () => {
    const position = defaultPipPosition({ width: 900, height: 700 });
    expect(position.x).toBe(900 - PIP_WIDTH - PIP_MARGIN);
    expect(position.y).toBe(PIP_MARGIN);
  });

  test("clamps dragged positions inside the pane", () => {
    expect(clampPipPosition(-40, -10, { width: 800, height: 600 })).toEqual({
      x: PIP_MARGIN,
      y: PIP_MARGIN,
    });
    const lowerRight = clampPipPosition(4000, 4000, { width: 800, height: 600 });
    expect(lowerRight.x).toBe(800 - PIP_WIDTH - PIP_MARGIN);
    expect(lowerRight.y).toBe(600 - PIP_MIN_HEIGHT - PIP_MARGIN);
  });

  test("stays inside a shrinking pane", () => {
    const position = clampPipPosition(500, 20, { width: 360, height: 280 });
    expect(position.x).toBeGreaterThanOrEqual(PIP_MARGIN);
    expect(position.x + 160).toBeLessThanOrEqual(360);
    expect(position.y).toBeGreaterThanOrEqual(PIP_MARGIN);
  });
});
