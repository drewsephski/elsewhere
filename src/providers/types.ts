import type { ModelDescriptor } from "@desktop/lib/definitions";

export interface GenerateMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface GenerateInput {
  model: string;
  messages: GenerateMessage[];
  requestId: string;
}

export type StreamEventType = "delta" | "done" | "error" | "cancelled" | "message";

export interface ProviderStreamEvent {
  type: StreamEventType;
  requestId: string;
  assistantMessageId: string;
  delta?: string;
  error?: string;
  fullContent?: string;
  message?: import("@desktop/lib/definitions").Message;
}

export interface ModelProvider {
  id: string;
  listModels(): Promise<ModelDescriptor[]>;
  startChat(params: {
    botId: string;
    conversationId?: string;
    content: string;
    requestId: string;
  }): Promise<{
    requestId: string;
    conversationId: string;
    userMessageId: string;
    assistantMessageId: string;
  }>;
  cancel(requestId: string): Promise<void>;
}
