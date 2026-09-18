import { describe, expect, it } from "vitest";
import {
  BOT_MODELS,
  DEFAULT_BOT_MODEL_ID,
  botModelById,
  botModelLabel,
  botModelsForSelect,
} from "./bot-models";

describe("bot models", () => {
  it("keeps Luna as the default and lists Astra, Sol, and Terra", () => {
    expect(DEFAULT_BOT_MODEL_ID).toBe("gpt-5.6-luna");
    expect(BOT_MODELS[0]?.id).toBe(DEFAULT_BOT_MODEL_ID);
    expect(BOT_MODELS.map((model) => model.id)).toEqual([
      "gpt-5.6-luna",
      "gpt-5.6-terra",
      "gpt-5.6-sol",
      "gpt-6-astra",
    ]);
    expect(botModelLabel(DEFAULT_BOT_MODEL_ID)).toBe("5.6 Luna");
    expect(botModelLabel("gpt-5.6-sol")).toBe("5.6 Sol");
    expect(botModelLabel("gpt-6-astra")).toBe("Astra 6");
  });

  it("keeps an unknown assigned model selectable", () => {
    expect(botModelById("codex")).toBeUndefined();
    expect(botModelLabel("codex")).toBe("codex");
    expect(botModelsForSelect("codex").map((model) => model.id)).toContain("codex");
    expect(botModelsForSelect("gpt-5.6-luna")).toHaveLength(BOT_MODELS.length);
  });
});
