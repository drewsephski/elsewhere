const IMAGE_EXTENSIONS: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  svg: "image/svg+xml",
};

export const RESULT_PREVIEW_MAX_BYTES = 1024 * 1024;

export function mimeTypeForResultFileName(name: string): string | null {
  const ext = name.split(".").pop()?.toLowerCase();
  if (!ext) {
    return null;
  }
  return IMAGE_EXTENSIONS[ext] ?? null;
}

export function isPreviewableImageFileName(name: string): boolean {
  return mimeTypeForResultFileName(name) !== null;
}

export function resultDownloadUrl(resultId: string, inline = false): string {
  const base = `/api/results/${resultId}/download`;
  return inline ? `${base}?inline=1` : base;
}
