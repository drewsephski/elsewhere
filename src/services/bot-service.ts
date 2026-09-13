import type { Bot } from "@/lib/definitions";
import { DEMO_AGENT_INPUT, isDemoAgent } from "@/lib/demo-agent";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { tauriApi, type CreateBotInput, type UpdateBotInput } from "@/lib/tauri-api";
import { localBotStore } from "@/services/local-bot-store";

async function bootstrapInTauri(): Promise<Bot[]> {
  try {
    return await tauriApi.bootstrapBots();
  } catch {
    const existing = await tauriApi.listBots();
    if (existing.some((bot) => isDemoAgent(bot))) {
      return existing;
    }
    await tauriApi.createBot(DEMO_AGENT_INPUT);
    return tauriApi.listBots();
  }
}

export const botService = {
  async bootstrap(): Promise<Bot[]> {
    if (isTauriRuntime()) {
      return bootstrapInTauri();
    }
    return localBotStore.bootstrap();
  },
  async list(includeArchived = false): Promise<Bot[]> {
    if (isTauriRuntime()) {
      return tauriApi.listBots(includeArchived);
    }
    return localBotStore.list(includeArchived);
  },
  async create(input: CreateBotInput): Promise<Bot> {
    if (isTauriRuntime()) {
      return tauriApi.createBot(input);
    }
    return localBotStore.create(input);
  },
  async update(input: UpdateBotInput): Promise<Bot> {
    if (isTauriRuntime()) {
      return tauriApi.updateBot(input);
    }
    return localBotStore.update(input);
  },
  async archive(id: string): Promise<void> {
    if (isTauriRuntime()) {
      return tauriApi.archiveBot(id);
    }
    localBotStore.archive(id);
  },
  async delete(id: string): Promise<void> {
    if (isTauriRuntime()) {
      return tauriApi.deleteBot(id);
    }
    localBotStore.delete(id);
  },
};
