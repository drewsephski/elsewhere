"use client";

import { Button } from "@/components/ui/button";

interface BotPresetPromptsProps {
  prompts: readonly string[];
  onSelect: (prompt: string) => void;
  disabled?: boolean;
}

export function BotPresetPrompts({
  prompts,
  onSelect,
  disabled = false,
}: BotPresetPromptsProps) {
  if (prompts.length === 0) {
    return null;
  }

  return (
    <ul
      className="flex flex-wrap justify-center gap-1.5 px-1"
      aria-label="Suggested prompts"
    >
      {prompts.map((prompt) => (
        <li key={prompt}>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={disabled}
            onClick={() => onSelect(prompt)}
            className="h-7 max-w-full rounded-full border-border bg-card px-3 text-[12px] font-normal text-muted-foreground hover:bg-surface-hover hover:text-foreground"
          >
            {prompt}
          </Button>
        </li>
      ))}
    </ul>
  );
}
