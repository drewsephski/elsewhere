import { tauriApi } from "@/lib/tauri-api";
import type { ModelProvider } from "@/providers/types";

export const openAiProvider: ModelProvider = {
  id: "openai",
  async listModels() {
    return tauriApi.listOpenAiModels();
  },
  async startChat(params) {
    return tauriApi.startChat({
      botId: params.botId,
      conversationId: params.conversationId,
      content: params.content,
      requestId: params.requestId,
    });
  },
  async cancel(requestId) {
    await tauriApi.cancelChat(requestId);
  },
};

export function getProvider(providerId: string): ModelProvider {
  if (providerId === "openai") {
    return openAiProvider;
  }
  throw new Error(`Unknown provider: ${providerId}`);
}
