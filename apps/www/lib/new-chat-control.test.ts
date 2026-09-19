import { describe, expect, it } from "vitest";
import { NEW_CHAT_ACTIVE_WORK_HINT, newChatButtonState } from "./new-chat-control";

describe("new chat control", () => {
  it("stays available when the Bot is idle", () => {
    expect(newChatButtonState({ starting: false, hasActiveWork: false })).toEqual({
      disabled: false,
      label: "New chat",
    });
  });

  it("blocks starting a new chat while work is queued or running", () => {
    expect(newChatButtonState({ starting: false, hasActiveWork: true })).toEqual({
      disabled: true,
      label: NEW_CHAT_ACTIVE_WORK_HINT,
    });
  });
});
