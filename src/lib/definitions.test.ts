import { describe, expect, test } from "vitest";
import {
  DEFAULT_MODEL_ID,
  isLunaModelAvailable,
  modelDisplayName,
  resolveDefaultModelId,
} from "@desktop/lib/definitions";

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

  test("labels Astra, Sol, Terra, and Luna for the picker", () => {
    expect(modelDisplayName("gpt-5.6-luna")).toBe("5.6 Luna");
    expect(modelDisplayName("gpt-5.6-sol")).toBe("5.6 Sol");
    expect(modelDisplayName("gpt-6-astra")).toBe("Astra 6");
    expect(modelDisplayName("gpt-4o-mini")).toBe("gpt-4o-mini");
  });
});
