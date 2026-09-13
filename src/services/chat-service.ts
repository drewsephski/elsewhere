import type { Conversation, Message } from "@/lib/definitions";
import { getProvider } from "@/providers/openai-provider";
import { tauriApi } from "@/lib/tauri-api";

export const chatService = {
  async loadConversation(botId: string): Promise<{
    conversation: Conversation;
    messages: Message[];
  }> {
    const conversation = await tauriApi.getOrCreateConversation(botId);
    const messages = await tauriApi.listMessages(conversation.id);
    return { conversation, messages };
  },

  async sendMessage(params: {
    botId: string;
    providerId: string;
    conversationId?: string;
    content: string;
  }) {
    const provider = getProvider(params.providerId);
    return provider.startChat({
      botId: params.botId,
      conversationId: params.conversationId,
      content: params.content,
    });
  },

  cancel(providerId: string, requestId: string) {
    return getProvider(providerId).cancel(requestId);
  },
};
