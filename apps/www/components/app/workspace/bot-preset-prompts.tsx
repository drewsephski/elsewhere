"use client";

import type { BotPresetPrompt } from "@/lib/bot-preset-prompts";
import { Suggestion, Suggestions } from "@/components/ai-elements/suggestion";

interface BotPresetPromptsProps {
  prompts: readonly BotPresetPrompt[];
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
    <Suggestions className="justify-center" aria-label="Suggested prompts" role="list">
      {prompts.map((item) => (
        <div key={item.label} role="listitem">
          <Suggestion
            suggestion={item.prompt}
            disabled={disabled}
            onClick={onSelect}
          >
            {item.label}
          </Suggestion>
        </div>
      ))}
    </Suggestions>
  );
}
