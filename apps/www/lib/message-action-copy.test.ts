import { describe, expect, it } from "vitest";
import { messageActionCopy } from "./message-action-copy";

describe("messageActionCopy", () => {
  it("describes recoverable archive for work runs", () => {
    const copy = messageActionCopy("archive");
    expect(copy.label).toBe("Archive");
    expect(copy.destructive).toBe(false);
    expect(copy.confirmMessage).toContain("Archived");
  });

  it("uses destructive delete copy for transcript messages", () => {
    const copy = messageActionCopy("delete");
    expect(copy.label).toBe("Delete");
    expect(copy.destructive).toBe(true);
  });
});
