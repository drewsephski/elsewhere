import { botAvatarImage } from "@/lib/bot-avatars";
import { getBotCreatureShellClass } from "@/lib/bot-visual";
import { cn } from "cn";

export type BotCreatureAvatarSize = "xs" | "sm" | "md" | "lg" | "xl" | "2xl";

const SIZE_CLASS: Record<BotCreatureAvatarSize, string> = {
  xs: "size-5",
  sm: "size-8",
  md: "size-10",
  lg: "size-12",
  xl: "size-14",
  "2xl": "size-16",
};

interface BotCreatureAvatarProps {
  name: string;
  avatarId?: string | null;
  /** Direct illustration src, used by the marketing demo. */
  src?: string;
  size?: BotCreatureAvatarSize;
  className?: string;
  /** Subtle bounce while the bot is responding */
  animated?: boolean;
  /** Colored tile behind the creature (legacy). Default: transparent. */
  showShell?: boolean;
  /**
   * `tile` clips the portrait into a compact round mark for nav stacks.
   */
  variant?: "plain" | "tile";
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

  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 items-end justify-center",
        showShell
          ? "overflow-hidden rounded-2xl ring-1 ring-inset"
          : tile
            ? "overflow-hidden rounded-full"
            : "overflow-visible bg-transparent",
        SIZE_CLASS[size],
        shell,
        className,
      )}
      role="img"
      aria-label={`${name.trim() || "Assistant"} avatar`}
    >
      <img
        src={image}
        alt=""
        width={280}
        height={320}
        decoding="async"
        draggable={false}
        className={cn(
          "max-h-full w-auto max-w-full object-contain object-bottom",
          animated && "animate-[bot-bob_2.4s_ease-in-out_infinite]",
        )}
        aria-hidden
      />
    </span>
  );
}
