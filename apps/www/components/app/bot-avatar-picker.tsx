"use client";

import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { BOT_AVATAR_PRESETS } from "@/lib/bot-avatars";
import { cn } from "cn";

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
  return (
    <div className={cn(compact ? "space-y-1.5" : "space-y-2", className)}>
      <p className={cn("font-medium", compact ? "text-xs" : "text-sm")}>Avatar</p>
      <div
        className={cn(
          "grid grid-cols-4 sm:grid-cols-6",
          compact ? "max-h-[7.25rem] gap-1 overflow-y-auto pr-0.5" : "gap-2",
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
                "flex flex-col items-center rounded-xl transition-colors",
                compact ? "gap-0 p-1" : "gap-0.5 p-1.5",
                selected
                  ? "bg-primary/10 ring-2 ring-primary/35"
                  : "hover:bg-muted/80 ring-1 ring-transparent",
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
            </button>
          );
        })}
      </div>
    </div>
  );
}
