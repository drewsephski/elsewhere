"use client";

import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { BOT_AVATAR_PRESETS } from "@/lib/bot-avatars";
import { cn } from "cn";

interface BotAvatarPickerProps {
  value: string;
  onChange: (avatarId: string) => void;
  disabled?: boolean;
  className?: string;
}

export function BotAvatarPicker({
  value,
  onChange,
  disabled,
  className,
}: BotAvatarPickerProps) {
  return (
    <div className={cn("space-y-2", className)}>
      <p className="text-sm font-medium">Avatar</p>
      <div
        className="grid grid-cols-4 gap-2 sm:grid-cols-6"
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
                "flex flex-col items-center gap-0.5 rounded-xl p-1.5 transition-colors",
                selected
                  ? "bg-primary/10 ring-2 ring-primary/35"
                  : "hover:bg-muted/80 ring-1 ring-transparent",
              )}
            >
              <BotCreatureAvatar name={preset.suggestedName} avatarId={preset.id} size="md" />
              <span className="w-full truncate text-center text-[10px] font-medium text-foreground">
                {preset.suggestedName}
              </span>
              <span className="w-full truncate text-center text-[9px] text-muted-foreground">
                {preset.label}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
