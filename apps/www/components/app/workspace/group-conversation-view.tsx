"use client";

import type { GroupConversationDetail, TranscriptMessage } from "@/lib/api-types";
import type { WorkspaceBotPresence } from "@/lib/workspace-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { UserPromptBubble } from "@/components/app/user-prompt-bubble";
import { MarkdownContent } from "@/components/app/markdown-content";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { Button } from "@/components/ui/button";
import { ChevronLeft } from "@/components/icons/lucide";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

interface GroupConversationViewProps {
  groupId: string;
  bots: WorkspaceBotPresence[];
}

export function GroupConversationView({ groupId, bots }: GroupConversationViewProps) {
  const [group, setGroup] = useState<GroupConversationDetail | null>(null);
  const [messages, setMessages] = useState<TranscriptMessage[]>([]);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const activeParticipants = useMemo(
    () => group?.participants.filter((p) => !p.leftAt) ?? [],
    [group],
  );

  const loadGroup = useCallback(async () => {
    const response = await cloudHostFetch(`/v1/conversations/${groupId}`);
    if (!response.ok) {
      throw new Error("Group not found");
    }
    setGroup(await response.json());
  }, [groupId]);

  const loadMessages = useCallback(async () => {
    const response = await cloudHostFetch(`/v1/conversations/${groupId}/messages`);
    if (!response.ok) {
      throw new Error("Could not load transcript");
    }
    setMessages(await response.json());
  }, [groupId]);

  useEffect(() => {
    void (async () => {
      try {
        await loadGroup();
        await loadMessages();
      } catch (err) {
        setError(err instanceof Error ? err.message : "Could not load group");
      }
    })();
  }, [loadGroup, loadMessages]);

  useEffect(() => {
    const timer = setInterval(() => {
      void loadMessages().catch(() => undefined);
    }, 5000);
    return () => clearInterval(timer);
  }, [loadMessages]);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!message.trim() || pending) {
      return;
    }
    setPending(true);
    setError(null);
    const trimmed = message.trim();
    setMessage("");
    try {
      const response = await cloudHostFetch(`/v1/conversations/${groupId}/messages`, {
        method: "POST",
        body: JSON.stringify({ body: trimmed }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not send message");
      }
      await loadMessages();
    } catch (err) {
      setMessage(trimmed);
      setError(err instanceof Error ? err.message : "Could not send message");
    } finally {
      setPending(false);
    }
  }

  async function handleRemoveParticipant(botId: string) {
    setError(null);
    const response = await cloudHostFetch(
      `/v1/conversations/${groupId}/participants/${botId}`,
      { method: "DELETE" },
    );
    const body = await response.json();
    if (!response.ok) {
      setError(body.error ?? "Could not remove participant");
      return;
    }
    setGroup(body);
  }

  async function handleAddParticipant(botId: string) {
    setError(null);
    const response = await cloudHostFetch(`/v1/conversations/${groupId}/participants`, {
      method: "POST",
      body: JSON.stringify({ botId }),
    });
    const body = await response.json();
    if (!response.ok) {
      setError(body.error ?? "Could not add participant");
      return;
    }
    setGroup(body);
  }

  const addableBots = bots.filter(
    (bot) => !activeParticipants.some((participant) => participant.botId === bot.id),
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex shrink-0 items-center justify-between gap-3 border-b border-border/70 bg-white/80 px-4 py-3 backdrop-blur-md">
        <div className="flex min-w-0 items-center gap-2.5">
          <Link
            href="/app"
            className="flex size-9 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-muted lg:hidden"
            aria-label="Back to bots"
          >
            <ChevronLeft className="size-5" />
          </Link>
          <div className="flex -space-x-2">
            {activeParticipants.slice(0, 4).map((participant) => (
              <BotCreatureAvatar
                key={participant.botId}
                name={participant.name}
                avatarId={participant.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                size="sm"
                className="ring-2 ring-white"
              />
            ))}
          </div>
          <div className="min-w-0">
            <p className="truncate font-semibold">{group?.name ?? "Group"}</p>
            <p className="text-xs text-muted-foreground">
              {activeParticipants.length} participants
            </p>
          </div>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-5">
        <div className="mx-auto flex max-w-2xl flex-col gap-4">
          {messages.map((item) =>
            item.authorKind === "human" ? (
              <UserPromptBubble key={item.id} sentAt={item.createdAt}>
                {item.body}
              </UserPromptBubble>
            ) : (
              <div key={item.id} className="flex justify-start">
                <div className="max-w-[90%] rounded-3xl rounded-bl-md border border-border/80 bg-white px-4 py-3 text-sm shadow-sm">
                  <div className="mb-2 flex items-center gap-2">
                    <BotCreatureAvatar
                      name={item.authorBotName ?? "Bot"}
                      avatarId={item.authorAvatarId ?? DEFAULT_BOT_AVATAR_ID}
                      size="sm"
                    />
                    <p className="text-xs font-medium text-muted-foreground">
                      {item.authorBotName ?? "Bot"}
                    </p>
                  </div>
                  <MarkdownContent text={item.body || (item.status === "pending" ? "Working…" : "")} />
                </div>
              </div>
            ),
          )}
        </div>
      </div>

      <footer className="shrink-0 space-y-3 border-t border-border/70 bg-white/90 px-4 py-3 backdrop-blur-md">
        <div className="mx-auto flex max-w-2xl flex-wrap items-center gap-2">
          {activeParticipants.map((participant) => (
            <button
              key={participant.botId}
              type="button"
              className="rounded-full border border-border px-2 py-1 text-xs hover:bg-muted"
              onClick={() => void handleRemoveParticipant(participant.botId)}
              aria-label={`Remove ${participant.name} from group`}
            >
              {participant.name} ×
            </button>
          ))}
          {addableBots.length > 0 && activeParticipants.length < 6 ? (
            <select
              className="rounded-full border border-border px-2 py-1 text-xs"
              defaultValue=""
              onChange={(event) => {
                const botId = event.target.value;
                if (botId) {
                  void handleAddParticipant(botId);
                  event.target.value = "";
                }
              }}
              aria-label="Add participant"
            >
              <option value="">Add bot…</option>
              {addableBots.map((bot) => (
                <option key={bot.id} value={bot.id}>{bot.name}</option>
              ))}
            </select>
          ) : null}
        </div>
        <form
          onSubmit={(event) => void handleSubmit(event)}
          className="mx-auto flex max-w-2xl items-end gap-2"
        >
          <textarea
            value={message}
            onChange={(event) => setMessage(event.target.value)}
            disabled={pending}
            rows={1}
            placeholder="Message the group"
            className="max-h-32 min-h-[2.25rem] flex-1 resize-none rounded-full border border-border/80 bg-[#f5f3f8] px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-primary/20"
            aria-label="Group message"
          />
          <Button type="submit" disabled={pending || !message.trim()}>
            {pending ? "Sending…" : "Send"}
          </Button>
        </form>
        {error ? (
          <p className="mx-auto max-w-2xl text-xs text-red-700" role="alert">{error}</p>
        ) : null}
      </footer>
    </div>
  );
}
