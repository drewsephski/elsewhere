export const BOT_ONBOARDING_OFFER_KEY = "elsewhere:bot-onboarding-offer";

export type BotOnboardingStatus =
  | "not_started"
  | "in_progress"
  | "ready_to_apply"
  | "completed"
  | "dismissed";

export type BotOnboardingOption = {
  id: string;
  label: string;
  description?: string;
};

export type BotOnboardingQuestion = {
  id: string;
  prompt: string;
  helper?: string;
  options: BotOnboardingOption[];
  allowCustom: boolean;
  index: number;
  maxQuestions: number;
};

export type BotOnboardingDraft = {
  summary: string;
  owns?: string;
  workingStyle?: string;
  instructions: string;
  context: string;
  suggestedFirstTasks: string[];
  suggestedCapabilities: string[];
};

export type BotOnboardingState = {
  botId: string;
  status: BotOnboardingStatus;
  questionsAsked: number;
  maxQuestions: number;
  revision: number;
  currentQuestion?: BotOnboardingQuestion | null;
  answers: Array<{ questionId: string; optionId?: string; customText?: string; label: string }>;
  draft?: BotOnboardingDraft | null;
  lastError?: string | null;
  generationModel: string;
};

export function botConversationHref(botId: string, options?: { setup?: boolean }): string {
  return options?.setup ? `/app/bots/${botId}?setup=1` : `/app/bots/${botId}`;
}

export function rememberOnboardingOffer(botId: string) {
  sessionStorage.setItem(BOT_ONBOARDING_OFFER_KEY, JSON.stringify({ botId }));
}

export function consumeOnboardingOffer(botId: string): boolean {
  const stored = sessionStorage.getItem(BOT_ONBOARDING_OFFER_KEY);
  if (!stored) {
    return false;
  }
  try {
    const parsed = JSON.parse(stored) as { botId?: unknown };
    if (parsed.botId !== botId) {
      return false;
    }
    sessionStorage.removeItem(BOT_ONBOARDING_OFFER_KEY);
    return true;
  } catch {
    sessionStorage.removeItem(BOT_ONBOARDING_OFFER_KEY);
    return false;
  }
}

export function parseOnboardingState(body: unknown): BotOnboardingState | null {
  if (!body || typeof body !== "object") {
    return null;
  }
  const value = body as Record<string, unknown>;
  if (typeof value.botId !== "string" || typeof value.status !== "string") {
    return null;
  }
  return {
    botId: value.botId,
    status: value.status as BotOnboardingStatus,
    questionsAsked: typeof value.questionsAsked === "number" ? value.questionsAsked : 0,
    maxQuestions: typeof value.maxQuestions === "number" ? value.maxQuestions : 3,
    revision: typeof value.revision === "number" ? value.revision : 0,
    currentQuestion: (value.currentQuestion as BotOnboardingQuestion | undefined) ?? null,
    answers: Array.isArray(value.answers)
      ? (value.answers as BotOnboardingState["answers"])
      : [],
    draft: (value.draft as BotOnboardingDraft | undefined) ?? null,
    lastError: typeof value.lastError === "string" ? value.lastError : null,
    generationModel: typeof value.generationModel === "string" ? value.generationModel : "",
  };
}
