import type { Bot } from "@/lib/definitions";
import {
  DEMO_AGENT_INPUT,
  isDemoAgent,
} from "@/lib/demo-agent";
import type { CreateBotInput, UpdateBotInput } from "@/lib/tauri-api";

const STORAGE_KEY = "gptbot.local-bots.v1";

function nowMs(): number {
  return Date.now();
}

function readAllBots(): Bot[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as Bot[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function writeAllBots(bots: Bot[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(bots));
}

function createBotRecord(input: CreateBotInput): Bot {
  const timestamp = nowMs();
  return {
    id: crypto.randomUUID(),
    name: input.name.trim(),
    description: input.description ?? null,
    systemPrompt: input.systemPrompt ?? "",
    provider: input.provider ?? "openai",
    model: input.model.trim(),
    computerEnabled: input.computerEnabled ?? true,
    createdAt: timestamp,
    updatedAt: timestamp,
    archivedAt: null,
  };
}

function ensureDemoInStore(bots: Bot[]): Bot[] {
  const hasActiveDemo = bots.some(
    (bot) => !bot.archivedAt && isDemoAgent(bot),
  );
  if (hasActiveDemo) {
    return bots;
  }
  return [...bots, createBotRecord(DEMO_AGENT_INPUT)];
}

function listActive(bots: Bot[]): Bot[] {
  return bots.filter((bot) => bot.archivedAt === null);
}

export const localBotStore = {
  bootstrap(): Bot[] {
    const withDemo = ensureDemoInStore(readAllBots());
    writeAllBots(withDemo);
    return listActive(withDemo);
  },

  list(includeArchived = false): Bot[] {
    const bots = readAllBots();
    if (includeArchived) {
      return bots;
    }
    return listActive(bots);
  },

  create(input: CreateBotInput): Bot {
    const bots = readAllBots();
    const created = createBotRecord(input);
    writeAllBots([...bots, created]);
    return created;
  },

  update(input: UpdateBotInput): Bot {
    const bots = readAllBots();
    const index = bots.findIndex((bot) => bot.id === input.id);
    if (index < 0) {
      throw new Error("Bot not found");
    }
    const existing = bots[index];
    const updated: Bot = {
      ...existing,
      name: input.name?.trim() || existing.name,
      description:
        input.description !== undefined ? input.description : existing.description,
      systemPrompt: input.systemPrompt ?? existing.systemPrompt,
      model: input.model?.trim() || existing.model,
      updatedAt: nowMs(),
    };
    const next = [...bots];
    next[index] = updated;
    writeAllBots(next);
    return updated;
  },

  archive(id: string): void {
    const bots = readAllBots();
    const index = bots.findIndex((bot) => bot.id === id);
    if (index < 0) {
      throw new Error("Bot not found");
    }
    const next = [...bots];
    next[index] = {
      ...next[index],
      archivedAt: nowMs(),
      updatedAt: nowMs(),
    };
    writeAllBots(next);
  },

  delete(id: string): void {
    writeAllBots(readAllBots().filter((bot) => bot.id !== id));
  },
};
