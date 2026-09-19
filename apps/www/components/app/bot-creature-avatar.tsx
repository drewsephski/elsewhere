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

function CreatureWorkingAura({ intensity }: { intensity: CreatureWorkingIntensity }) {
  return (
    <span data-creature-aura data-intensity={intensity} className="pointer-events-none" aria-hidden>
      <span className="creature-halo creature-halo-a" />
      <span className="creature-halo creature-halo-b" />
      <span className="creature-halo creature-halo-c" />
      <span className="creature-working-static-ring" />
      <svg className="creature-rings" viewBox="0 0 100 100">
        <ellipse
          className="creature-ring creature-ring-spin"
          cx="50"
          cy="56"
          rx="36"
          ry="20"
          fill="none"
        />
        {intensity !== "compact" ? (
          <ellipse
            className="creature-ring creature-ring-reverse"
            cx="50"
            cy="50"
            rx="22"
            ry="32"
            fill="none"
          />
        ) : null}
      </svg>
      <span className="creature-spark" />
      {intensity !== "compact" ? <span className="creature-spark creature-spark-b" /> : null}
      {intensity === "hero" ? <span className="creature-sheen" /> : null}
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
  const colors = animated ? getBotAvatarPreset(avatarId).colors : null;
  const workingStyle = colors
    ? ({
        "--creature-accent": colors.accent,
        "--creature-body": colors.body,
      } as CSSProperties)
    : undefined;

  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 items-end justify-center",
        animated && "isolate",
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
    >
      {animated ? <CreatureWorkingAura intensity={WORKING_INTENSITY[size]} /> : null}
      <img
        src={image}
        alt=""
        width={280}
        height={320}
        decoding="async"
        draggable={false}
        className={cn(
          "relative z-[1] max-h-full w-auto max-w-full object-contain object-bottom",
          animated && "creature-breathe motion-reduce:animate-none",
        )}
        aria-hidden
      />
    </span>
  );
}
