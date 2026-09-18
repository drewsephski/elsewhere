import { describe, expect, it } from "vitest";
import { BOT_AVATAR_PRESETS } from "@/lib/bot-avatars";
import { botPresetPrompts } from "@/lib/bot-preset-prompts";

function labels(bot: { avatarId?: string | null; name?: string | null }) {
  return botPresetPrompts(bot).map((item) => item.label);
}

describe("botPresetPrompts", () => {
  it("covers every picker bot with three distinct short labels and richer prompts", () => {
    for (const preset of BOT_AVATAR_PRESETS) {
      const prompts = botPresetPrompts({ avatarId: preset.id });
      expect(prompts).toHaveLength(3);
      expect(new Set(prompts.map((item) => item.label)).size).toBe(3);
      expect(new Set(prompts.map((item) => item.prompt)).size).toBe(3);
      for (const item of prompts) {
        expect(item.prompt.length).toBeGreaterThan(item.label.length);
      }
    }
  });

  it("tailors prompt labels to the bot role", () => {
    expect(labels({ avatarId: "amber-puff" })).toContain("Write a sourced brief");
    expect(labels({ avatarId: "teal-wisp" })).toContain("Inspect the workspace");
    expect(labels({ avatarId: "violet-kitty" })).toContain("Critique the live UI");
    expect(labels({ avatarId: "berry-puff" })).toContain("Prep a briefing");
  });

  it("fills the composer with a more specific researcher prompt", () => {
    const sourced = botPresetPrompts({ avatarId: "amber-puff" }).find(
      (item) => item.label === "Write a sourced brief",
    );
    expect(sourced?.prompt).toContain("cite");
    expect(sourced?.prompt).toContain("[topic]");
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
    expect(labels({ avatarId: "sky-wisp" })).toContain("Clarify the goal");
    expect(labels({ avatarId: "lime-sprout" })).toContain("Troubleshoot this");
  });

  it("uses generic starters for custom bots", () => {
    expect(labels({ name: "Scout", avatarId: "unknown-bot" })).toEqual([
      "What can you take on?",
      "Look around the computer",
      "Propose a first task",
    ]);
  });
});
