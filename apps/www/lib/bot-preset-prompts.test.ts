import { describe, expect, it } from "vitest";
import { BOT_AVATAR_PRESETS } from "@/lib/bot-avatars";
import { botPresetPrompts } from "@/lib/bot-preset-prompts";

describe("botPresetPrompts", () => {
  it("covers every picker bot with three distinct prompts", () => {
    for (const preset of BOT_AVATAR_PRESETS) {
      const prompts = botPresetPrompts({ avatarId: preset.id });
      expect(prompts).toHaveLength(3);
      expect(new Set(prompts).size).toBe(3);
    }
  });

  it("tailors prompts to the bot role", () => {
    expect(botPresetPrompts({ avatarId: "amber-puff" })).toContain("Write a sourced brief");
    expect(botPresetPrompts({ avatarId: "teal-wisp" })).toContain("Inspect the workspace");
    expect(botPresetPrompts({ avatarId: "violet-kitty" })).toContain("Critique the live UI");
    expect(botPresetPrompts({ avatarId: "berry-puff" })).toContain("Prep a briefing");
  });

  it("falls back to the role name when no avatar is stored", () => {
    expect(botPresetPrompts({ name: "Writer" })).toEqual(
      botPresetPrompts({ avatarId: "rose-sprout" }),
    );
    expect(botPresetPrompts({ name: "Chief of Staff" })).toEqual(
      botPresetPrompts({ avatarId: "berry-puff" }),
    );
  });

  it("keeps retired avatars on their original role", () => {
    expect(botPresetPrompts({ avatarId: "sky-wisp" })).toContain("Clarify the goal");
    expect(botPresetPrompts({ avatarId: "lime-sprout" })).toContain("Troubleshoot this");
  });

  it("uses generic starters for custom bots", () => {
    expect(botPresetPrompts({ name: "Scout", avatarId: "unknown-bot" })).toEqual([
      "What can you take on?",
      "Look around the computer",
      "Propose a first task",
    ]);
  });
});
