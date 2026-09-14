import type { Bot } from "@/lib/definitions";
import { DEFAULT_MODEL_ID } from "@/lib/definitions";
import type { CreateBotInput } from "@/lib/tauri-api";
import { botService } from "@/services/bot-service";

export const DEMO_AGENT_NAME = "Scout";

const DEMO_AGENT_SYSTEM_PROMPT = `You are Scout, a calm and capable research assistant inside Elsewhere.

Help the user explore ideas, compare options, and turn curiosity into clear next steps. Be concise unless they ask for depth. Prefer structured answers with short headings and bullet points when useful.

When information might be outdated, say what you know and what you would verify. Never invent sources.`;

export const DEMO_AGENT_INPUT: CreateBotInput = {
  name: DEMO_AGENT_NAME,
  description: "Research & discovery",
  systemPrompt: DEMO_AGENT_SYSTEM_PROMPT,
  provider: "openai",
  model: DEFAULT_MODEL_ID,
  computerEnabled: false,
};

export function isDemoAgent(bot: Pick<Bot, "name">): boolean {
  return bot.name === DEMO_AGENT_NAME;
}

export function sortBotsForNav(bots: Bot[]): Bot[] {
  return [...bots].sort((a, b) => {
    const aDemo = isDemoAgent(a) ? 0 : 1;
    const bDemo = isDemoAgent(b) ? 0 : 1;
    if (aDemo !== bDemo) {
      return aDemo - bDemo;
    }
    return b.updatedAt - a.updatedAt;
  });
}

export async function ensureDemoAgent(): Promise<void> {
  await botService.bootstrap();
}
