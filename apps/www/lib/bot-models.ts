export type BotModel = {
  id: string;
  label: string;
  blurb: string;
};

/** Authoritative default model for new Bots. */
export const DEFAULT_BOT_MODEL_ID = "gpt-5.6-luna";

/** Curated ChatGPT / Codex models a Bot can be assigned. */
export const BOT_MODELS: readonly BotModel[] = [
  {
    id: DEFAULT_BOT_MODEL_ID,
    label: "5.6 Luna",
    blurb: "Fast and affordable. Best for clear, repeatable work.",
  },
  {
    id: "gpt-5.6-terra",
    label: "5.6 Terra",
    blurb: "Balanced intelligence and cost for everyday work.",
  },
  {
    id: "gpt-5.6-sol",
    label: "5.6 Sol",
    blurb: "Flagship GPT-5.6 for complex professional work.",
  },
  {
    id: "gpt-6-astra",
    label: "Astra 6",
    blurb: "Strongest model for hard end-to-end work.",
  },
];

export function botModelById(id: string | null | undefined): BotModel | undefined {
  if (!id) {
    return undefined;
  }
  return BOT_MODELS.find((model) => model.id === id);
}

export function botModelLabel(id: string | null | undefined): string {
  return botModelById(id)?.label ?? (id?.trim() || botModelLabel(DEFAULT_BOT_MODEL_ID));
}

export function botModelsForSelect(currentId?: string | null): BotModel[] {
  const models = [...BOT_MODELS];
  const current = currentId?.trim();
  if (current && !models.some((model) => model.id === current)) {
    models.push({
      id: current,
      label: current,
      blurb: "Currently assigned to this Bot.",
    });
  }
  return models;
}
