import { describe, expect, test } from "vitest";
import {
  DEFAULT_MODEL_ID,
  isLunaModelAvailable,
  resolveDefaultModelId,
} from "@/lib/definitions";

describe("Luna default model policy", () => {
  test("DEFAULT_MODEL_ID is gpt-5.6-luna", () => {
    expect(DEFAULT_MODEL_ID).toBe("gpt-5.6-luna");
  });

  test("resolveDefaultModelId prefers Luna when listed", () => {
    expect(
      resolveDefaultModelId([
        { id: "gpt-4o-mini" },
        { id: "gpt-5.6-luna" },
      ]),
    ).toBe("gpt-5.6-luna");
  });

  test("does not fall back to models[0] when Luna is missing", () => {
    expect(
      resolveDefaultModelId([{ id: "gpt-4o-mini" }, { id: "o3-mini" }]),
    ).toBe("gpt-5.6-luna");
    expect(isLunaModelAvailable([{ id: "gpt-4o-mini" }])).toBe(false);
  });
});
