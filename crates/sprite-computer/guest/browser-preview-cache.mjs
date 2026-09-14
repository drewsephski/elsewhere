import fs from "fs";
import path from "path";
import crypto from "crypto";

/** @typedef {{ available: boolean, url: string | null, title: string | null }} PreviewVisibleMeta */

export const PREVIEW_META_NAME = "meta.json";
export const PREVIEW_FRAMES_DIR_NAME = "frames";
export const MAX_RETAINED_FRAMES = 8;

export function sha256Hex(buffer) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

/**
 * Strong frame identity: JPEG bytes plus visible URL/title so identical pixels at
 * a different location do not share an ETag.
 * @param {Buffer | null} imageBuffer
 * @param {PreviewVisibleMeta} visible
 */
export function computeFrameEtag(imageBuffer, visible) {
  const visibilityPayload = JSON.stringify({
    url: visible.url ?? null,
    title: visible.title ?? null,
    available: visible.available,
  });
  if (!imageBuffer || imageBuffer.length === 0) {
    return sha256Hex(Buffer.from(visibilityPayload));
  }
  const imageHash = sha256Hex(imageBuffer);
  const visibilityHash = sha256Hex(Buffer.from(visibilityPayload));
  return sha256Hex(Buffer.from(`${imageHash}:${visibilityHash}`));
}

function fsyncFileBestEffort(filePath) {
  try {
    const fd = fs.openSync(filePath, "r");
    try {
      fs.fsyncSync(fd);
    } finally {
      fs.closeSync(fd);
    }
  } catch {
    // best-effort on platforms without fsync support
  }
}

function frameFileName(etag) {
  return `${etag}.jpg`;
}

function resolveFramePath(previewDir, frameFile) {
  const normalized = path.normalize(path.join(previewDir, frameFile));
  const framesRoot = path.join(previewDir, PREVIEW_FRAMES_DIR_NAME);
  if (!normalized.startsWith(framesRoot + path.sep)) {
    throw new Error("invalid preview frame path");
  }
  return normalized;
}

/**
 * @param {string} previewDir
 * @param {string | null | undefined} framePath
 */
export function readPreviewImage(previewDir, framePath) {
  if (!framePath) {
    return null;
  }
  try {
    return fs.readFileSync(resolveFramePath(previewDir, framePath));
  } catch {
    return null;
  }
}

/**
 * @param {string} previewDir
 */
export function loadPreviewVersionFromDisk(previewDir) {
  const metaPath = path.join(previewDir, PREVIEW_META_NAME);
  try {
    const meta = JSON.parse(fs.readFileSync(metaPath, "utf8"));
    if (typeof meta.version === "number" && meta.version >= 0) {
      return meta.version;
    }
  } catch {
    // no cache yet
  }
  return 0;
}

/**
 * @param {string} previewDir
 * @param {string} keepFramePath
 */
export function pruneRetainedFrames(previewDir, keepFramePath) {
  const framesDir = path.join(previewDir, PREVIEW_FRAMES_DIR_NAME);
  let entries;
  try {
    entries = fs.readdirSync(framesDir);
  } catch {
    return;
  }
  const keepName = keepFramePath ? path.basename(keepFramePath) : null;
  const files = entries
    .filter((name) => name.endsWith(".jpg"))
    .map((name) => {
      const full = path.join(framesDir, name);
      try {
        return { name, mtimeMs: fs.statSync(full).mtimeMs };
      } catch {
        return null;
      }
    })
    .filter(Boolean)
    .sort((a, b) => b.mtimeMs - a.mtimeMs);

  const retain = new Set();
  if (keepName) {
    retain.add(keepName);
  }
  for (const file of files) {
    if (retain.size >= MAX_RETAINED_FRAMES) {
      break;
    }
    retain.add(file.name);
  }

  for (const file of files) {
    if (retain.has(file.name)) {
      continue;
    }
    try {
      fs.unlinkSync(path.join(framesDir, file.name));
    } catch {
      // ignore
    }
  }
}

/**
 * @param {string} previewDir
 * @param {number} previewVersion
 * @param {PreviewVisibleMeta & { contentType?: string }} metaFields
 * @param {Buffer | null} imageBuffer
 */
export function writePreviewCacheAtomic(previewDir, previewVersion, metaFields, imageBuffer) {
  fs.mkdirSync(previewDir, { recursive: true, mode: 0o700 });
  const framesDir = path.join(previewDir, PREVIEW_FRAMES_DIR_NAME);
  fs.mkdirSync(framesDir, { recursive: true, mode: 0o700 });

  const nextVersion = previewVersion + 1;
  const capturedAt = new Date().toISOString();
  const visible = {
    available: metaFields.available,
    url: metaFields.url ?? null,
    title: metaFields.title ?? null,
  };
  const etag = computeFrameEtag(imageBuffer, visible);

  let framePath = null;
  if (imageBuffer && metaFields.available) {
    const frameFile = path.join(PREVIEW_FRAMES_DIR_NAME, frameFileName(etag));
    const frameAbs = resolveFramePath(previewDir, frameFile);
    if (!fs.existsSync(frameAbs)) {
      const frameTmp = `${frameAbs}.tmp.${process.pid}`;
      fs.writeFileSync(frameTmp, imageBuffer);
      fsyncFileBestEffort(frameTmp);
      fs.renameSync(frameTmp, frameAbs);
      fsyncFileBestEffort(frameAbs);
    }
    framePath = frameFile;
  }

  const fullMeta = {
    version: nextVersion,
    etag,
    framePath,
    capturedAt,
    contentType: metaFields.contentType ?? "image/jpeg",
    ...visible,
  };

  const metaPath = path.join(previewDir, PREVIEW_META_NAME);
  const metaTmp = `${metaPath}.tmp.${process.pid}`;
  fs.writeFileSync(metaTmp, JSON.stringify(fullMeta));
  fsyncFileBestEffort(metaTmp);
  fs.renameSync(metaTmp, metaPath);
  fsyncFileBestEffort(metaPath);

  pruneRetainedFrames(previewDir, framePath);

  return { version: nextVersion, etag, framePath, capturedAt, fullMeta };
}

/**
 * @param {string} previewDir
 */
export function readPreviewCacheFromDisk(previewDir) {
  const metaPath = path.join(previewDir, PREVIEW_META_NAME);
  let meta;
  try {
    meta = JSON.parse(fs.readFileSync(metaPath, "utf8"));
  } catch {
    return { ok: true, available: false, version: 0 };
  }

  const version =
    typeof meta.version === "number" && meta.version >= 0 ? meta.version : 0;
  if (!meta?.available) {
    return {
      ok: true,
      available: false,
      url: meta.url ?? null,
      title: meta.title ?? null,
      version,
      etag: meta.etag ?? null,
    };
  }

  const imageBuffer = readPreviewImage(previewDir, meta.framePath);
  if (!imageBuffer) {
    return {
      ok: true,
      available: false,
      url: meta.url ?? null,
      version,
      etag: meta.etag ?? null,
    };
  }

  return {
    ok: true,
    available: true,
    url: meta.url ?? null,
    title: meta.title ?? null,
    contentType: meta.contentType ?? "image/jpeg",
    imageBase64: imageBuffer.toString("base64"),
    version,
    etag: meta.etag ?? null,
    framePath: meta.framePath ?? null,
    capturedAt: meta.capturedAt ?? null,
  };
}
