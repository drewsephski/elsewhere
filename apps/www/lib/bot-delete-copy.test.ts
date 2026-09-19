import { describe, expect, it } from "vitest";
import { BOT_DELETE_COPY } from "./bot-delete-copy";

describe("bot delete copy", () => {
  it("describes destructive delete without claiming work history is kept", () => {
    expect(BOT_DELETE_COPY).toContain("Deletes this Bot");
    expect(BOT_DELETE_COPY).toContain("group conversations may remain");
    expect(BOT_DELETE_COPY.toLowerCase()).not.toContain("work history may remain");
  });
});
