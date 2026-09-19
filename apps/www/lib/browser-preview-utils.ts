export function previewHostname(url: string | null): string | null {
  if (!url) {
    return null;
  }
  try {
    return new URL(url).hostname;
  } catch {
    return null;
  }
}

export function clampUnitRatio(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.min(1, Math.max(0, value));
}

export function containedImagePointerRatio(
  clientX: number,
  clientY: number,
  container: Pick<DOMRect, "left" | "top" | "width" | "height">,
  naturalWidth: number,
  naturalHeight: number,
): { x: number; y: number } | null {
  if (container.width <= 0 || container.height <= 0) {
    return null;
  }
  if (!naturalWidth || !naturalHeight) {
    return {
      x: clampUnitRatio((clientX - container.left) / container.width),
      y: clampUnitRatio((clientY - container.top) / container.height),
    };
  }
  const scale = Math.min(container.width / naturalWidth, container.height / naturalHeight);
  const contentWidth = naturalWidth * scale;
  const contentHeight = naturalHeight * scale;
  const offsetX = container.left + (container.width - contentWidth) / 2;
  const offsetY = container.top + (container.height - contentHeight) / 2;
  if (contentWidth <= 0 || contentHeight <= 0) {
    return null;
  }
  const x = (clientX - offsetX) / contentWidth;
  const y = (clientY - offsetY) / contentHeight;
  if (x < 0 || x > 1 || y < 0 || y > 1) {
    return null;
  }
  return { x, y };
}

export function normalizeWheelDelta(deltaX: number, deltaY: number, deltaMode: number): {
  x: number;
  y: number;
} {
  const factor = deltaMode === 1 ? 16 : deltaMode === 2 ? 600 : 1;
  const clamp = (value: number) => Math.max(-2400, Math.min(2400, value));
  return {
    x: clamp(deltaX * factor),
    y: clamp(deltaY * factor),
  };
}
