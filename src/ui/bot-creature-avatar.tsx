import { useId } from "react";
import type { BotCreatureKind, BotCreatureSpec } from "@desktop/lib/bot-visual";
import { getBotCreatureShellClass, getBotCreatureSpec } from "@desktop/lib/bot-visual";
import { cn } from "@desktop/lib/utils";

export type BotCreatureAvatarSize = "sm" | "md" | "lg";

const SIZE_CLASS: Record<BotCreatureAvatarSize, string> = {
  sm: "size-6",
  md: "size-8",
  lg: "size-10",
};

interface BotCreatureAvatarProps {
  name: string;
  size?: BotCreatureAvatarSize;
  className?: string;
  /** Subtle bounce while the bot is responding */
  animated?: boolean;
}

function CreatureFeatures({
  kind,
  colors,
}: {
  kind: BotCreatureKind;
  colors: BotCreatureSpec["colors"];
}) {
  switch (kind) {
    case "bunny":
      return (
        <>
          <ellipse cx="22" cy="14" rx="7" ry="13" fill={colors.body} />
          <ellipse cx="42" cy="14" rx="7" ry="13" fill={colors.body} />
          <ellipse cx="22" cy="16" rx="4" ry="8" fill={colors.cheek} opacity={0.55} />
          <ellipse cx="42" cy="16" rx="4" ry="8" fill={colors.cheek} opacity={0.55} />
        </>
      );
    case "kitty":
      return (
        <>
          <path d="M18 22 L14 8 L26 18 Z" fill={colors.bodyDark} />
          <path d="M46 22 L50 8 L38 18 Z" fill={colors.bodyDark} />
          <line
            x1="12"
            y1="34"
            x2="20"
            y2="33"
            stroke={colors.pupil}
            strokeWidth="1.2"
            strokeLinecap="round"
            opacity={0.35}
          />
          <line
            x1="44"
            y1="33"
            x2="52"
            y2="34"
            stroke={colors.pupil}
            strokeWidth="1.2"
            strokeLinecap="round"
            opacity={0.35}
          />
        </>
      );
    case "sprout":
      return (
        <>
          <path
            d="M32 10 C28 4 22 6 24 14 C26 10 30 12 32 10 Z"
            fill={colors.accent}
          />
          <path
            d="M32 10 C36 4 42 6 40 14 C38 10 34 12 32 10 Z"
            fill={colors.accent}
            opacity={0.85}
          />
          <ellipse cx="32" cy="12" rx="3" ry="2" fill={colors.belly} opacity={0.6} />
        </>
      );
    case "wisp":
      return (
        <>
          <ellipse
            cx="18"
            cy="36"
            rx="9"
            ry="12"
            fill={colors.wing}
            opacity={0.75}
            transform="rotate(-18 18 36)"
          />
          <ellipse
            cx="46"
            cy="36"
            rx="9"
            ry="12"
            fill={colors.wing}
            opacity={0.75}
            transform="rotate(18 46 36)"
          />
          <circle cx="32" cy="9" r="3.5" fill={colors.accent} />
          <circle cx="32" cy="9" r="1.5" fill={colors.belly} opacity={0.9} />
        </>
      );
    case "puff":
      return (
        <>
          <ellipse cx="24" cy="54" rx="5" ry="3" fill={colors.bodyDark} opacity={0.35} />
          <ellipse cx="40" cy="54" rx="5" ry="3" fill={colors.bodyDark} opacity={0.35} />
        </>
      );
  }
}

function CreatureFace({ spec }: { spec: BotCreatureSpec }) {
  const { colors, expression, eyeScale, blush, freckle } = spec;
  const eyeY = 31 + expression * 2;
  const eyeR = 5.2 * eyeScale;
  const pupilR = 2.4 * eyeScale;
  const highlightR = 1.1;

  return (
    <>
      {blush && (
        <>
          <ellipse cx="21" cy="38" rx="4" ry="2.5" fill={colors.cheek} opacity={0.45} />
          <ellipse cx="43" cy="38" rx="4" ry="2.5" fill={colors.cheek} opacity={0.45} />
        </>
      )}
      <circle cx="26" cy={eyeY} r={eyeR} fill="#FFFFFF" />
      <circle cx="38" cy={eyeY} r={eyeR} fill="#FFFFFF" />
      <circle cx="26" cy={eyeY + 0.5} r={pupilR} fill={colors.pupil} />
      <circle cx="38" cy={eyeY + 0.5} r={pupilR} fill={colors.pupil} />
      <circle cx="27.8" cy={eyeY - 1.2} r={highlightR} fill="#FFFFFF" opacity={0.95} />
      <circle cx="39.8" cy={eyeY - 1.2} r={highlightR} fill="#FFFFFF" opacity={0.95} />
      {freckle && (
        <>
          <circle cx="30" cy="40" r="0.8" fill={colors.accent} opacity={0.5} />
          <circle cx="34" cy="41" r="0.7" fill={colors.accent} opacity={0.45} />
        </>
      )}
      <path
        d={`M28 ${42 + expression} Q32 ${45 + expression * 1.5} 36 ${42 + expression}`}
        fill="none"
        stroke={colors.pupil}
        strokeWidth="1.4"
        strokeLinecap="round"
        opacity={0.55}
      />
    </>
  );
}

function CreatureSvg({
  spec,
  animated,
  gradientId,
}: {
  spec: BotCreatureSpec;
  animated?: boolean;
  gradientId: string;
}) {
  const { kind, colors } = spec;

  return (
    <svg
      viewBox="0 0 64 64"
      className={cn("size-full", animated && "animate-[bot-bob_2.4s_ease-in-out_infinite]")}
      aria-hidden
    >
      <defs>
        <linearGradient id={gradientId} x1="32" y1="14" x2="32" y2="56">
          <stop offset="0%" stopColor={colors.body} />
          <stop offset="100%" stopColor={colors.bodyDark} />
        </linearGradient>
      </defs>
      <ellipse cx="32" cy="54" rx="16" ry="4" fill={colors.pupil} opacity={0.08} />
      <CreatureFeatures kind={kind} colors={colors} />
      <ellipse cx="32" cy="38" rx="22" ry="20" fill={`url(#${gradientId})`} />
      <ellipse cx="32" cy="42" rx="14" ry="11" fill={colors.belly} opacity={0.85} />
      <CreatureFace spec={spec} />
      {kind === "kitty" && (
        <ellipse cx="52" cy="44" rx="4" ry="3" fill={colors.bodyDark} opacity={0.4} />
      )}
    </svg>
  );
}

export function BotCreatureAvatar({
  name,
  size = "md",
  className,
  animated = false,
}: BotCreatureAvatarProps) {
  const spec = getBotCreatureSpec(name);
  const shell = getBotCreatureShellClass(name);
  const gradientId = useId().replace(/:/g, "");

  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full ring-1 ring-inset",
        SIZE_CLASS[size],
        shell,
        className,
      )}
      role="img"
      aria-label={`${name.trim() || "Assistant"} avatar`}
    >
      <CreatureSvg spec={spec} animated={animated} gradientId={gradientId} />
    </span>
  );
}
