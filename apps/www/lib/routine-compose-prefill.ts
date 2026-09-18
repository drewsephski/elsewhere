import type { BotSummary, ConversationSummary } from "@/lib/api-types";

export interface RoutineComposePrefill {
  botId: string;
  destinationConversationId: string;
}

export function resolveRoutineComposePrefill(
  bots: BotSummary[],
  conversations: ConversationSummary[],
  botId: string | null,
  conversationId: string | null,
): RoutineComposePrefill | null {
  if (!botId?.trim()) {
    return null;
  }
  const bot = bots.find((row) => row.id === botId);
  if (!bot) {
    return null;
  }

  let destinationConversationId = "";
  if (conversationId?.trim()) {
    const conversation = conversations.find((row) => row.id === conversationId);
    if (conversation?.conversationType === "group") {
      destinationConversationId = conversation.id;
    } else if (conversation?.botId === bot.id) {
      destinationConversationId = conversation.id;
    }
  }

  return { botId: bot.id, destinationConversationId };
}
