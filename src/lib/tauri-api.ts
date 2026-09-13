import { invoke } from "@tauri-apps/api/core";
import type {
  Bot,
  Conversation,
  Message,
  ModelDescriptor,
} from "@/lib/definitions";

export interface CreateBotInput {
  name: string;
  description?: string | null;
  systemPrompt?: string;
  provider?: string;
  model: string;
}

export interface UpdateBotInput {
  id: string;
  name?: string;
  description?: string | null;
  systemPrompt?: string;
  model?: string;
}

export interface StartChatInput {
  botId: string;
  conversationId?: string;
  content: string;
}

export interface StartChatResult {
  requestId: string;
  conversationId: string;
  userMessageId: string;
  assistantMessageId: string;
}

export const tauriApi = {
  listBots(includeArchived = false): Promise<Bot[]> {
    return invoke("list_bots", { includeArchived });
  },
  getBot(id: string): Promise<Bot> {
    return invoke("get_bot", { id });
  },
  createBot(input: CreateBotInput): Promise<Bot> {
    return invoke("create_bot", { input });
  },
  updateBot(input: UpdateBotInput): Promise<Bot> {
    return invoke("update_bot", { input });
  },
  archiveBot(id: string): Promise<void> {
    return invoke("archive_bot", { id });
  },
  deleteBot(id: string): Promise<void> {
    return invoke("delete_bot", { id });
  },
  listConversations(botId: string): Promise<Conversation[]> {
    return invoke("list_conversations", { botId });
  },
  getOrCreateConversation(botId: string): Promise<Conversation> {
    return invoke("get_or_create_conversation", { botId });
  },
  listMessages(conversationId: string): Promise<Message[]> {
    return invoke("list_messages", { conversationId });
  },
  getApiKeyStatus(): Promise<{ configured: boolean }> {
    return invoke("get_api_key_status");
  },
  setOpenAiApiKey(apiKey: string): Promise<void> {
    return invoke("set_openai_api_key", { apiKey });
  },
  clearOpenAiApiKey(): Promise<void> {
    return invoke("clear_openai_api_key");
  },
  listOpenAiModels(): Promise<ModelDescriptor[]> {
    return invoke("list_openai_models");
  },
  startChat(input: StartChatInput): Promise<StartChatResult> {
    return invoke("start_chat", { input });
  },
  cancelChat(requestId: string): Promise<void> {
    return invoke("cancel_chat", { requestId });
  },
};
