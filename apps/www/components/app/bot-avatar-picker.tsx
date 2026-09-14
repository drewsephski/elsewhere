"use client";

import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { BOT_AVATAR_PRESETS } from "@/lib/bot-avatars";
import { cn } from "cn";
import { LayoutGroup, motion, useReducedMotion } from "motion/react";

const AVATAR_SELECTION_LAYOUT_ID = "bot-avatar-selection";

const selectionSpring = {
  type: "spring" as const,
  stiffness: 420,
  damping: 34,
  mass: 0.75,
};

interface BotAvatarPickerProps {
  value: string;
  onChange: (avatarId: string) => void;
  disabled?: boolean;
  className?: string;
  /** Tighter grid for dialogs and narrow layouts. */
  compact?: boolean;
}

export function BotAvatarPicker({
  value,
  onChange,
  disabled,
  className,
  compact,
}: BotAvatarPickerProps) {
  const reduceMotion = useReducedMotion();

  return (
    <div className={cn(compact ? "space-y-1.5" : "space-y-2", className)}>
      <p className={cn("font-medium", compact ? "text-xs" : "text-sm")}>Avatar</p>
      <LayoutGroup id="bot-avatar-picker">
        <div
          className={cn(
            "grid grid-cols-4 sm:grid-cols-6",
            compact
              ? "max-h-[8rem] gap-1.5 overflow-y-auto overscroll-contain p-1.5 [-ms-overflow-style:none] [scrollbar-width:thin]"
              : "gap-2.5 p-1",
          )}
          role="radiogroup"
          aria-label="Choose bot avatar"
        >
          {BOT_AVATAR_PRESETS.map((preset) => {
            const selected = preset.id === value;
            return (
              <button
                key={preset.id}
                type="button"
                role="radio"
                aria-checked={selected}
                disabled={disabled}
                title={preset.suggestedName}
                onClick={() => onChange(preset.id)}
                className={cn(
                  "relative flex flex-col items-center rounded-xl outline-none transition-colors",
                  "focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-card",
                  compact ? "gap-0 p-1.5" : "gap-0.5 p-2",
                  !selected && "hover:bg-muted/70",
                )}
              >
                {selected ? (
                  <motion.span
                    layoutId={AVATAR_SELECTION_LAYOUT_ID}
                    className="pointer-events-none absolute inset-0 rounded-xl border-2 border-primary/45 bg-primary/10 shadow-[inset_0_0_0_1px] shadow-primary/10"
                    transition={
                      reduceMotion
                        ? { duration: 0 }
                        : selectionSpring
                    }
                  />
                ) : null}
                <span
                  className={cn(
                    "relative z-10 flex w-full flex-col items-center",
                    compact ? "gap-0" : "gap-0.5",
                  )}
                >
                  <BotCreatureAvatar
                    name={preset.suggestedName}
                    avatarId={preset.id}
                    size={compact ? "sm" : "md"}
                  />
                  <span
                    className={cn(
                      "w-full truncate text-center font-medium text-foreground",
                      compact ? "text-[9px] leading-tight" : "text-[10px]",
                    )}
                  >
                    {preset.suggestedName}
                  </span>
                  {!compact ? (
                    <span className="w-full truncate text-center text-[9px] text-muted-foreground">
                      {preset.label}
                    </span>
                  ) : null}
                </span>
              </button>
            );
          })}
        </div>
      </LayoutGroup>
    </div>
  );
}
