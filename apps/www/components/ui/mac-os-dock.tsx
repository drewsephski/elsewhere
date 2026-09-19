"use client";

import { cn } from "cn";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
} from "react";

export interface DockApp {
  id: string;
  name: string;
  icon: string;
}

export type MacOSDockSize = "mini" | "compact" | "default";

export interface MacOSDockProps {
  apps: DockApp[];
  onAppClick: (appId: string) => void;
  openApps?: string[];
  busyApps?: string[];
  disabled?: boolean;
  size?: MacOSDockSize;
  className?: string;
}

interface DockDensity {
  baseIconSize: number;
  maxScale: number;
  spacing: number;
  padX: number;
  padY: number;
  effectWidth: number;
}

const SIZE_PRESETS: Record<MacOSDockSize, Omit<DockDensity, "spacing" | "padX" | "padY" | "effectWidth">> = {
  mini: { baseIconSize: 24, maxScale: 1.42 },
  compact: { baseIconSize: 28, maxScale: 1.46 },
  default: { baseIconSize: 40, maxScale: 1.6 },
};

const MIN_SCALE = 1;

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return true;
  }
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function densityForWidth(size: MacOSDockSize, appCount: number, availableWidth: number | null): DockDensity {
  const preset = SIZE_PRESETS[size];
  let baseIconSize = preset.baseIconSize;
  let maxScale = preset.maxScale;
  let spacing = Math.max(3, baseIconSize * 0.18);
  let padX = Math.max(8, baseIconSize * 0.38);
  const padY = Math.max(5, baseIconSize * 0.18);

  if (availableWidth && availableWidth > 0 && appCount > 0) {
    const restWidth = appCount * baseIconSize + (appCount - 1) * spacing + padX * 2;
    if (restWidth > availableWidth) {
      const scale = Math.max(0.58, (availableWidth - 4) / restWidth);
      baseIconSize = Math.max(16, baseIconSize * scale);
      spacing = Math.max(2, spacing * scale);
      padX = Math.max(6, padX * scale);
      maxScale = Math.min(maxScale, 1.28 + 0.16 * scale);
    }
  }

  return {
    baseIconSize,
    maxScale,
    spacing,
    padX,
    padY,
    effectWidth: Math.max(64, baseIconSize * 2.8),
  };
}

function calculateScales(
  mouseX: number | null,
  appCount: number,
  density: DockDensity,
): number[] {
  if (mouseX === null || prefersReducedMotion()) {
    return Array.from({ length: appCount }, () => MIN_SCALE);
  }

  return Array.from({ length: appCount }, (_, index) => {
    const iconCenter = index * (density.baseIconSize + density.spacing) + density.baseIconSize / 2;
    const minX = mouseX - density.effectWidth / 2;
    const maxX = mouseX + density.effectWidth / 2;
    if (iconCenter < minX || iconCenter > maxX) {
      return MIN_SCALE;
    }
    const theta = ((iconCenter - minX) / density.effectWidth) * 2 * Math.PI;
    const scaleFactor = (1 - Math.cos(Math.min(Math.max(theta, 0), 2 * Math.PI))) / 2;
    return MIN_SCALE + scaleFactor * (density.maxScale - MIN_SCALE);
  });
}

function calculatePositions(scales: number[], density: DockDensity): number[] {
  let currentX = 0;
  return scales.map((scale) => {
    const scaledWidth = density.baseIconSize * scale;
    const centerX = currentX + scaledWidth / 2;
    currentX += scaledWidth + density.spacing;
    return centerX;
  });
}

function bounceIcon(element: HTMLElement, distance: number) {
  if (prefersReducedMotion() || typeof element.animate !== "function") {
    return;
  }
  element.animate(
    [
      { transform: "translateY(0)" },
      { transform: `translateY(${distance}px)` },
      { transform: "translateY(0)" },
    ],
    { duration: 420, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
  );
}

export function MacOSDock({
  apps,
  onAppClick,
  openApps = [],
  busyApps = [],
  disabled = false,
  size = "default",
  className,
}: MacOSDockProps) {
  const measureRef = useRef<HTMLDivElement>(null);
  const dockRef = useRef<HTMLElement>(null);
  const iconRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const mouseXRef = useRef<number | null>(null);
  const scalesRef = useRef<number[]>(apps.map(() => MIN_SCALE));
  const positionsRef = useRef<number[]>([]);
  const frameRef = useRef<number | null>(null);
  const lastMoveRef = useRef(0);
  const [availableWidth, setAvailableWidth] = useState<number | null>(null);
  const [scales, setScales] = useState<number[]>(() => apps.map(() => MIN_SCALE));
  const [positions, setPositions] = useState<number[]>([]);

  const density = useMemo(
    () => densityForWidth(size, apps.length, availableWidth),
    [apps.length, availableWidth, size],
  );

  useEffect(() => {
    const node = measureRef.current;
    if (!node) {
      return;
    }

    function applyWidth(width: number) {
      setAvailableWidth((current) => (current !== null && Math.abs(current - width) < 0.5 ? current : width));
    }

    applyWidth(node.getBoundingClientRect().width);

    if (typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width;
      if (typeof width === "number") {
        applyWidth(width);
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const restScales = useMemo(() => apps.map(() => MIN_SCALE), [apps]);
  const restPositions = useMemo(
    () => calculatePositions(restScales, density),
    [density, restScales],
  );

  useEffect(() => {
    scalesRef.current = restScales;
    positionsRef.current = restPositions;
    setScales(restScales);
    setPositions(restPositions);
  }, [restPositions, restScales]);

  const stopLoop = useCallback(() => {
    if (frameRef.current !== null) {
      cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }
  }, []);

  const animateToTarget = useCallback(() => {
    const targetScales = calculateScales(mouseXRef.current, apps.length, density);
    const targetPositions = calculatePositions(targetScales, density);
    const lerp = mouseXRef.current !== null ? 0.24 : 0.16;
    const nextScales = scalesRef.current.map((current, index) => {
      const target = targetScales[index] ?? MIN_SCALE;
      return current + (target - current) * lerp;
    });
    const nextPositions = positionsRef.current.map((current, index) => {
      const target = targetPositions[index] ?? current;
      return current + (target - current) * lerp;
    });
    scalesRef.current = nextScales;
    positionsRef.current = nextPositions;
    setScales(nextScales);
    setPositions(nextPositions);

    const stillMoving =
      mouseXRef.current !== null ||
      nextScales.some((scale, index) => Math.abs(scale - (targetScales[index] ?? MIN_SCALE)) > 0.003) ||
      nextPositions.some((position, index) => Math.abs(position - (targetPositions[index] ?? position)) > 0.15);

    if (stillMoving && !prefersReducedMotion()) {
      frameRef.current = requestAnimationFrame(animateToTarget);
    } else {
      frameRef.current = null;
      if (mouseXRef.current === null) {
        scalesRef.current = restScales;
        positionsRef.current = restPositions;
        setScales(restScales);
        setPositions(restPositions);
      }
    }
  }, [apps.length, density, restPositions, restScales]);

  const startLoop = useCallback(() => {
    if (frameRef.current !== null || prefersReducedMotion()) {
      if (prefersReducedMotion()) {
        const targetScales = calculateScales(mouseXRef.current, apps.length, density);
        const targetPositions = calculatePositions(targetScales, density);
        scalesRef.current = targetScales;
        positionsRef.current = targetPositions;
        setScales(targetScales);
        setPositions(targetPositions);
      }
      return;
    }
    frameRef.current = requestAnimationFrame(animateToTarget);
  }, [animateToTarget, apps.length, density]);

  useEffect(() => () => stopLoop(), [stopLoop]);

  function handleMouseMove(event: ReactMouseEvent<HTMLElement>) {
    const now = performance.now();
    if (now - lastMoveRef.current < 16) {
      return;
    }
    lastMoveRef.current = now;
    const rect = dockRef.current?.getBoundingClientRect();
    if (!rect) {
      return;
    }
    mouseXRef.current = event.clientX - rect.left - density.padX;
    startLoop();
  }

  function handleMouseLeave() {
    mouseXRef.current = null;
    startLoop();
  }

  function handleAppActivate(appId: string, index: number) {
    if (disabled) {
      return;
    }
    const icon = iconRefs.current[index];
    if (icon) {
      bounceIcon(icon, Math.min(-6, -density.baseIconSize * 0.22));
    }
    onAppClick(appId);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>, appId: string, index: number) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      handleAppActivate(appId, index);
    }
  }

  const contentWidth =
    positions.length > 0
      ? Math.max(
          ...positions.map((position, index) => position + (density.baseIconSize * (scales[index] ?? MIN_SCALE)) / 2),
        )
      : apps.length * (density.baseIconSize + density.spacing) - density.spacing;

  const trayWidth = contentWidth + density.padX * 2;
  const trayHeight = density.baseIconSize * density.maxScale + density.padY * 2 + 10;

  return (
    <div ref={measureRef} className={cn("flex w-full justify-center", className)}>
      <nav
        ref={dockRef}
        role="toolbar"
        aria-label="Application dock"
        aria-disabled={disabled || undefined}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        onClick={(event) => event.stopPropagation()}
        onPointerDown={(event) => event.stopPropagation()}
        className={cn(
          "relative flex items-end overflow-visible rounded-[18px] border border-white/12 bg-[#1b1b1d]/78 shadow-[0_8px_24px_rgba(0,0,0,0.38)] backdrop-blur-xl",
          disabled && "opacity-55",
        )}
        style={{
          width: `${trayWidth}px`,
          height: `${trayHeight}px`,
          paddingLeft: density.padX,
          paddingRight: density.padX,
          paddingBottom: density.padY,
          maxWidth: "100%",
        }}
      >
        {apps.map((app, index) => {
          const scale = scales[index] ?? MIN_SCALE;
          const position = positions[index] ?? restPositions[index] ?? 0;
          const scaledSize = density.baseIconSize * scale;
          const isOpen = openApps.includes(app.id);
          const isBusy = busyApps.includes(app.id);
          return (
            <button
              key={app.id}
              type="button"
              ref={(node) => {
                iconRefs.current[index] = node;
              }}
              disabled={disabled}
              aria-label={app.name}
              aria-pressed={isOpen}
              aria-busy={isBusy || undefined}
              title={app.name}
              onClick={() => handleAppActivate(app.id, index)}
              onKeyDown={(event) => handleKeyDown(event, app.id, index)}
              className="group absolute flex flex-col items-center justify-end rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-white/80 focus-visible:ring-offset-2 focus-visible:ring-offset-transparent disabled:cursor-not-allowed"
              style={{
                left: `${density.padX + position - scaledSize / 2}px`,
                bottom: `${density.padY}px`,
                width: `${scaledSize}px`,
                height: `${scaledSize}px`,
                zIndex: Math.round(scale * 10),
              }}
            >
              <span className="pointer-events-none absolute -top-6 rounded-md bg-black/72 px-1.5 py-0.5 text-[9px] font-medium text-white/90 opacity-0 shadow-sm transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
                {app.name}
              </span>
              {/* eslint-disable-next-line @next/next/no-img-element */}
              <img
                src={app.icon}
                alt=""
                draggable={false}
                className={cn(
                  "h-full w-full rounded-[22%] object-cover",
                  isBusy && "animate-pulse opacity-80",
                )}
                style={{
                  filter: `drop-shadow(0 ${scale > 1.15 ? 2 : 1}px ${scale > 1.15 ? 4 : 2}px rgba(0,0,0,${0.22 + (scale - 1) * 0.12}))`,
                }}
              />
              <span
                className={cn(
                  "absolute -bottom-[5px] size-[3px] rounded-full bg-white/90",
                  isOpen ? "opacity-100" : "opacity-0",
                )}
                aria-hidden
              />
            </button>
          );
        })}
      </nav>
    </div>
  );
}
