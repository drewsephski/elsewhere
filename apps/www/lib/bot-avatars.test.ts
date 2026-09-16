import { describe, expect, it } from "vitest";
import {
  BOT_AVATAR_PRESETS,
  botAvatarImage,
  DEFAULT_BOT_AVATAR_ID,
  JOB_BOT_IMAGES,
} from "@/lib/bot-avatars";

describe("botAvatarImage", () => {
  it("maps known avatar ids to landing job-bot portraits", () => {
    expect(botAvatarImage("rose-sprout")).toBe(JOB_BOT_IMAGES.writer);
    expect(botAvatarImage("teal-wisp")).toBe(JOB_BOT_IMAGES.engineer);
    expect(botAvatarImage("violet-kitty")).toBe(JOB_BOT_IMAGES.designer);
    expect(botAvatarImage("berry-puff")).toBe(JOB_BOT_IMAGES.chiefOfStaff);
  });

  it("still resolves retired picker ids so existing bots keep a portrait", () => {
    expect(botAvatarImage("sky-wisp")).toBe(JOB_BOT_IMAGES.chiefOfStaff);
    expect(botAvatarImage("lime-sprout")).toBe(JOB_BOT_IMAGES.writer);
  });

  it("matches landing roster names when no avatar id is stored", () => {
    expect(botAvatarImage(null, "Writer")).toBe(JOB_BOT_IMAGES.writer);
    expect(botAvatarImage(undefined, "Researcher")).toBe(JOB_BOT_IMAGES.researcher);
    expect(botAvatarImage("", "Finance")).toBe(JOB_BOT_IMAGES.finance);
  });

  it("falls back to the default portrait", () => {
    expect(botAvatarImage(DEFAULT_BOT_AVATAR_ID)).toBe(JOB_BOT_IMAGES.chiefOfStaff);
    expect(botAvatarImage()).toBe(JOB_BOT_IMAGES.chiefOfStaff);
  });
});

describe("BOT_AVATAR_PRESETS", () => {
  it("exposes the eight landing job bots", () => {
    expect(BOT_AVATAR_PRESETS.map((preset) => preset.suggestedName)).toEqual([
      "Researcher",
      "Writer",
      "Analyst",
      "Finance",
      "Engineer",
      "Designer",
      "Marketing",
      "Chief of Staff",
    ]);
    expect(new Set(BOT_AVATAR_PRESETS.map((preset) => preset.image)).size).toBe(8);
    expect(BOT_AVATAR_PRESETS.some((preset) => preset.id === DEFAULT_BOT_AVATAR_ID)).toBe(
      true,
    );
  });
});
