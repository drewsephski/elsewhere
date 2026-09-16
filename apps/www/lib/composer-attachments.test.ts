import { describe, expect, it } from "vitest";
import {
  composerCanSend,
  formatFileSize,
  type StagedComposerFile,
} from "@/components/app/workspace/composer-attachments";

function file(status: StagedComposerFile["status"], name = "a.txt"): StagedComposerFile {
  return {
    localId: name,
    file: { name, size: 1, type: "text/plain" } as File,
    status,
    attachment:
      status === "ready"
        ? {
            id: "att-1",
            originalName: name,
            safeName: name,
            mimeType: "text/plain",
            sizeBytes: 1,
            sha256: "abc",
            kind: "text",
          }
        : undefined,
  };
}

describe("composer attachments", () => {
  it("enables send for text or a ready attachment, but not while uploading", () => {
    expect(composerCanSend("", [])).toBe(false);
    expect(composerCanSend("hello", [])).toBe(true);
    expect(composerCanSend("", [file("ready")])).toBe(true);
    expect(composerCanSend("hello", [file("uploading")])).toBe(false);
    expect(composerCanSend("", [file("error")])).toBe(false);
  });

  it("formats sizes", () => {
    expect(formatFileSize(512)).toBe("512 B");
    expect(formatFileSize(2048)).toBe("2.0 KB");
  });
});
