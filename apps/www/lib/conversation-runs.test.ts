import { describe, expect, it } from "vitest";
import {
  buildConversationRunsListPath,
  buildRunsListPreviewPath,
  retryPrefillText,
  truncateRunTaskPreview,
  RUN_TASK_LIST_PREVIEW_MAX,
} from "./conversation-runs";

describe("conversation runs list paths", () => {
  it("requests full_task for bot conversation history", () => {
    const path = buildConversationRunsListPath("bot_1", "conv_1");
    expect(path).toContain("full_task=true");
    expect(path).toContain("conversation_id=conv_1");
    expect(path).toContain("bot_id=bot_1");
  });

  it("does not request full_task for default work list previews", () => {
    const path = buildRunsListPreviewPath();
    expect(path).not.toContain("full_task");
  });
});

describe("truncateRunTaskPreview", () => {
  it("keeps preview bounded at 180 characters", () => {
    const long = "a".repeat(250);
    const preview = truncateRunTaskPreview(long);
    expect(preview.length).toBe(RUN_TASK_LIST_PREVIEW_MAX);
    expect(long.startsWith(preview)).toBe(true);
  });
});

describe("retryPrefillText", () => {
  it("restores the full task text without sending", () => {
    const full = `line-${"x".repeat(220)}-end`;
    expect(retryPrefillText({ status: "failed", task: full })).toBe(full);
    expect(retryPrefillText({ status: "failed", task: full }).length).toBeGreaterThan(
      RUN_TASK_LIST_PREVIEW_MAX,
    );
  });

  it("prefixes interrupted runs without truncating the stored task", () => {
    const full = "y".repeat(200);
    const text = retryPrefillText({ status: "interrupted", task: full });
    expect(text).toContain(full);
    expect(text.startsWith("Please continue where you left off:")).toBe(true);
  });
});
