import type { Bot } from "@/lib/definitions";
import { tauriApi, type CreateBotInput, type UpdateBotInput } from "@/lib/tauri-api";

export const botService = {
  list(includeArchived = false): Promise<Bot[]> {
    return tauriApi.listBots(includeArchived);
  },
  create(input: CreateBotInput): Promise<Bot> {
    return tauriApi.createBot(input);
  },
  update(input: UpdateBotInput): Promise<Bot> {
    return tauriApi.updateBot(input);
  },
  archive(id: string): Promise<void> {
    return tauriApi.archiveBot(id);
  },
  delete(id: string): Promise<void> {
    return tauriApi.deleteBot(id);
  },
};
