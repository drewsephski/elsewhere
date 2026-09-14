import fs from "fs";
import os from "os";
import path from "path";
import { describe, expect, it, beforeEach, afterEach } from "vitest";
import {
  computeFrameEtag,
  loadPreviewVersionFromDisk,
  readPreviewCacheFromDisk,
  writePreviewCacheAtomic,
  PREVIEW_FRAMES_DIR_NAME,
} from "./browser-preview-cache.mjs";

describe("browser preview content-addressed cache", () => {
  /** @type {string} */
  let previewDir;

  beforeEach(() => {
    previewDir = fs.mkdtempSync(path.join(os.tmpdir(), "elsewhere-preview-"));
  });

  afterEach(() => {
    fs.rmSync(previewDir, { recursive: true, force: true });
  });

  it("uses a new ETag when visible metadata changes but JPEG bytes are identical", () => {
    const jpeg = Buffer.from("fake-jpeg-bytes");
    const first = writePreviewCacheAtomic(
      previewDir,
      0,
      { available: true, url: "https://a.example", title: "A" },
      jpeg,
    );
    const second = writePreviewCacheAtomic(
      previewDir,
      first.version,
      { available: true, url: "https://b.example", title: "B" },
      jpeg,
    );
    expect(second.etag).not.toBe(first.etag);
    const readBack = readPreviewCacheFromDisk(previewDir);
    expect(readBack.available).toBe(true);
    expect(readBack.url).toBe("https://b.example");
    expect(readBack.etag).toBe(second.etag);
  });

  it("keeps the previous frame valid if a new image lands before metadata is updated", () => {
    const jpegA = Buffer.from("frame-a");
    const jpegB = Buffer.from("frame-b");
    writePreviewCacheAtomic(
      previewDir,
      0,
      { available: true, url: "https://stable.example", title: "Stable" },
      jpegA,
    );
    const metaBefore = JSON.parse(
      fs.readFileSync(path.join(previewDir, "meta.json"), "utf8"),
    );
    const interruptedEtag = computeFrameEtag(jpegB, {
      available: true,
      url: "https://next.example",
      title: "Next",
    });
    const framesDir = path.join(previewDir, PREVIEW_FRAMES_DIR_NAME);
    fs.mkdirSync(framesDir, { recursive: true });
    fs.writeFileSync(path.join(framesDir, `${interruptedEtag}.jpg`), jpegB);

    const readBack = readPreviewCacheFromDisk(previewDir);
    expect(readBack.available).toBe(true);
    expect(readBack.url).toBe("https://stable.example");
    expect(readBack.etag).toBe(metaBefore.etag);
    expect(readBack.framePath).toBe(metaBefore.framePath);
  });

  it("preserves frame identity across daemon restart (version reload)", () => {
    const jpeg = Buffer.from("restart-frame");
    const written = writePreviewCacheAtomic(
      previewDir,
      0,
      { available: true, url: "https://restart.example", title: "Restart" },
      jpeg,
    );
    const reloadedVersion = loadPreviewVersionFromDisk(previewDir);
    expect(reloadedVersion).toBe(written.version);
    const readBack = readPreviewCacheFromDisk(previewDir);
    expect(readBack.etag).toBe(written.etag);
    expect(readBack.version).toBe(written.version);
    expect(readBack.imageBase64).toBe(jpeg.toString("base64"));
  });
});
