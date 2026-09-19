import { botAvatarImage, getBotAvatarPreset } from "@/lib/bot-avatars";
import { getBotCreatureShellClass } from "@/lib/bot-visual";
import { cn } from "cn";
import type { CSSProperties } from "react";

export type BotCreatureAvatarSize = "xs" | "sm" | "md" | "lg" | "xl" | "2xl";

type CreatureWorkingIntensity = "compact" | "focus" | "hero";

const SIZE_CLASS: Record<BotCreatureAvatarSize, string> = {
  xs: "size-5",
  sm: "size-8",
  md: "size-10",
  lg: "size-12",
  xl: "size-14",
  "2xl": "size-16",
};

const WORKING_INTENSITY: Record<BotCreatureAvatarSize, CreatureWorkingIntensity> = {
  xs: "compact",
  sm: "focus",
  md: "hero",
  lg: "hero",
  xl: "hero",
  "2xl": "hero",
};

interface BotCreatureAvatarProps {
  name: string;
  avatarId?: string | null;
  /** Direct illustration src, used by the marketing demo. */
  src?: string;
  size?: BotCreatureAvatarSize;
  className?: string;
  animated?: boolean;
  /** Colored tile behind the creature (legacy). Default: transparent. */
  showShell?: boolean;
  /**
   * `tile` clips the portrait into a compact round mark for nav stacks.
   */
  variant?: "plain" | "tile";
}

function creaturePhaseDelay(seed: string): string {
  let hash = 2166136261;
  for (let i = 0; i < seed.length; i += 1) {
    hash ^= seed.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return `${-((hash >>> 0) % 2800) / 1000}s`;
}

function CreatureWorkingAura({ intensity }: { intensity: CreatureWorkingIntensity }) {
  return (
    <span data-creature-aura data-intensity={intensity} className="pointer-events-none" aria-hidden>
      <span className="creature-glow" />
      {intensity === "hero" ? <span className="creature-glow creature-glow-core" /> : null}
      {intensity !== "compact" ? <span className="creature-ground" /> : null}
    </span>
  );
}

export function BotCreatureAvatar({
  name,
  avatarId,
  src,
  size = "md",
  className,
  animated = false,
  showShell = false,
  variant = "plain",
}: BotCreatureAvatarProps) {
  const image = src ?? botAvatarImage(avatarId, name);
  const shell = showShell ? getBotCreatureShellClass(name) : "";
  const tile = variant === "tile";
  const intensity = animated ? WORKING_INTENSITY[size] : null;
  const colors = animated ? getBotAvatarPreset(avatarId).colors : null;
  const workingStyle = colors
    ? ({
        "--creature-accent": colors.accent,
        "--creature-body": colors.body,
        "--creature-phase": creaturePhaseDelay(avatarId ?? name),
      } as CSSProperties)
    : undefined;

  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 items-end justify-center",
        animated && "creature-stage isolate",
        showShell
          ? "overflow-hidden rounded-2xl ring-1 ring-inset"
          : tile
            ? "overflow-hidden rounded-full"
            : "overflow-visible bg-transparent",
        SIZE_CLASS[size],
        shell,
        className,
      )}
      style={workingStyle}
      role="img"
      aria-label={`${name.trim() || "Assistant"} avatar`}
      data-working={animated ? "on" : "off"}
      data-intensity={intensity ?? undefined}
    >
      {animated && intensity ? <CreatureWorkingAura intensity={intensity} /> : null}
      <span
        className={cn(
          "relative z-[1] flex h-full w-full items-end justify-center",
          animated && "creature-volume motion-reduce:animate-none",
        )}
      >
        <img
          src={image}
          alt=""
          width={280}
          height={320}
          decoding="async"
          draggable={false}
          className="max-h-full w-auto max-w-full object-contain object-bottom"
          aria-hidden
        />
        {animated && intensity !== "compact" ? (
          <span
            className="creature-spec"
            style={{
              WebkitMaskImage: `url(${image})`,
              maskImage: `url(${image})`,
            }}
          />
        ) : null}
      </span>
    </span>
  );
}
