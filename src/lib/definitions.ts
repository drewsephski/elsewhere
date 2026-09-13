import { z } from "zod";

/** Default OpenAI chat model for new bots and demos. */
export const DEFAULT_MODEL_ID = "gpt-5.6-luna";

export function resolveDefaultModelId(
  models: readonly { id: string }[],
): string {
  const preferred = models.find(
    (m) =>
      m.id.toLowerCase() === DEFAULT_MODEL_ID ||
      m.id.toLowerCase().includes("luna"),
  );
  return preferred?.id ?? models[0]?.id ?? DEFAULT_MODEL_ID;
}

export const messageRoleSchema = z.enum(["system", "user", "assistant"]);
export type MessageRole = z.infer<typeof messageRoleSchema>;

export const messageStatusSchema = z.enum([
  "pending",
  "streaming",
  "complete",
  "error",
  "cancelled",
  "interrupted",
]);

export const modelCapabilitiesSchema = z.object({
  textOutput: z.boolean(),
  streaming: z.boolean(),
  tools: z.boolean(),
  vision: z.boolean(),
});
export type ModelCapabilities = z.infer<typeof modelCapabilitiesSchema>;
export type MessageStatus = z.infer<typeof messageStatusSchema>;

export const botSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  systemPrompt: z.string(),
  provider: z.string(),
  model: z.string(),
  createdAt: z.number(),
  updatedAt: z.number(),
  archivedAt: z.number().nullable(),
});
export type Bot = z.infer<typeof botSchema>;

export const conversationSchema = z.object({
  id: z.string(),
  botId: z.string(),
  title: z.string().nullable(),
  createdAt: z.number(),
  updatedAt: z.number(),
});
export type Conversation = z.infer<typeof conversationSchema>;

export const messageSchema = z.object({
  id: z.string(),
  conversationId: z.string(),
  role: messageRoleSchema,
  kind: z.string(),
  body: z.string(),
  status: messageStatusSchema,
  model: z.string().nullable(),
  errorMessage: z.string().nullable(),
  createdAt: z.number(),
  updatedAt: z.number(),
});
export type Message = z.infer<typeof messageSchema>;

export const modelDescriptorSchema = z.object({
  id: z.string(),
  displayName: z.string(),
  provider: z.string(),
  capabilities: modelCapabilitiesSchema,
});
export type ModelDescriptor = z.infer<typeof modelDescriptorSchema>;

export const streamEventSchema = z.object({
  requestId: z.string(),
  eventType: z.string(),
  conversationId: z.string(),
  assistantMessageId: z.string(),
  delta: z.string().nullable().optional(),
  error: z.string().nullable().optional(),
  fullContent: z.string().nullable().optional(),
  message: messageSchema.nullable().optional(),
});
export type StreamEvent = z.infer<typeof streamEventSchema>;
