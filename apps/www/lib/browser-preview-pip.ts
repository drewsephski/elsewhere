export const PIP_WIDTH = 400;
export const PIP_MARGIN = 12;
export const PIP_MIN_HEIGHT = 240;

export interface PipBounds {
  width: number;
  height: number;
}

export interface PipPosition {
  x: number;
  y: number;
}

export interface PipSize {
  width: number;
  height: number;
}

export function clampPipPosition(
  x: number,
  y: number,
  bounds: PipBounds,
  size: PipSize = { width: PIP_WIDTH, height: PIP_MIN_HEIGHT },
): PipPosition {
  const width = Math.min(size.width, Math.max(160, bounds.width - PIP_MARGIN * 2));
  const height = Math.min(size.height, Math.max(120, bounds.height - PIP_MARGIN * 2));
  const maxX = Math.max(PIP_MARGIN, bounds.width - width - PIP_MARGIN);
  const maxY = Math.max(PIP_MARGIN, bounds.height - height - PIP_MARGIN);
  return {
    x: Math.min(Math.max(PIP_MARGIN, x), maxX),
    y: Math.min(Math.max(PIP_MARGIN, y), maxY),
  };
}

/** Upper-right of the chat pane, with margin from the top and trailing edge. */
export function defaultPipPosition(
  bounds: PipBounds,
  size: PipSize = { width: PIP_WIDTH, height: PIP_MIN_HEIGHT },
): PipPosition {
  const width = Math.min(size.width, Math.max(160, bounds.width - PIP_MARGIN * 2));
  return clampPipPosition(bounds.width - width - PIP_MARGIN, PIP_MARGIN, bounds, size);
}
